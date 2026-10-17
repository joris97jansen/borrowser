"""The complete run-owned infrastructure: one instance and one inline-rule SG."""
import base64
import gzip
import io
import json
import re
from pathlib import Path
from aws_cdk import CfnTag, Environment, Fn, LegacyStackSynthesizer, RemovalPolicy, Stack
from aws_cdk import aws_ec2 as ec2

ROOT = Path(__file__).resolve().parent


class RunnerStack(Stack):
    def __init__(self, app, request):
        b = request["binding"]
        config = request["network"]
        runner = json.loads((ROOT / "runner.json").read_text())
        if (runner["region"], runner["instance_type"], runner["root_gib"], runner["gp3_iops"],
            runner["gp3_throughput_mib"], runner["guest_stop_minutes"]) != ("eu-west-1", "c6i.large", 16, 3000, 125, 45):
            raise ValueError("unsupported AG4 resource limits")
        image = request["reviewed_image"]
        snapshot = request["ubuntu_snapshot"]
        if not re.fullmatch(r"ami-[0-9a-f]{17}", image["id"]):
            raise ValueError("literal reviewed AMI required")
        if not re.fullmatch(r"\d{8}T\d{6}Z", snapshot):
            raise ValueError("literal Ubuntu snapshot timestamp required")
        if b["region"] != "eu-west-1" or not re.fullmatch(r"[0-9a-f]{32}", b["run_id"]):
            raise ValueError("unsupported operation")
        if b["stack_name"] != "borrowser-ag4-" + b["run_id"]:
            raise ValueError("stack identity mismatch")
        if set(config) != {"vpc_id", "subnet_id", "endpoint_sg_id", "target_sg_id"}:
            raise ValueError("exact verified EICE network inputs required")
        for name, prefix in (("vpc_id", "vpc"), ("subnet_id", "subnet"),
                             ("endpoint_sg_id", "sg"), ("target_sg_id", "sg")):
            if not re.fullmatch(prefix + r"-[0-9a-f]{17}", config[name]):
                raise ValueError("invalid verified EICE network identity")
        if config["endpoint_sg_id"] == config["target_sg_id"]:
            raise ValueError("distinct endpoint and target groups required")
        key = request["ssh_public_key"].strip()
        if not re.fullmatch(r"ssh-ed25519 [A-Za-z0-9+/=]+(?: [A-Za-z0-9_-]+)?", key):
            raise ValueError("invalid operation SSH public key")
        super().__init__(app, b["stack_name"], stack_name=b["stack_name"],
                         env=Environment(account=b["account"], region=b["region"]),
                         synthesizer=LegacyStackSynthesizer(), analytics_reporting=False,
                         termination_protection=False)
        self.template_options.metadata = {"BorrowserAg4Operation": b}
        tags = [CfnTag(key="borrowser:purpose", value="ag4"),
                CfnTag(key="borrowser:run-id", value=b["run_id"]),
                CfnTag(key="borrowser:stack-id", value=self.stack_id)]
        name_prefix = "borrowser-chromium-comparison-"
        sg = ec2.CfnSecurityGroup(self, "RunnerSecurityGroup", vpc_id=config["vpc_id"],
            group_description="Disposable Borrowser Chromium comparison capture",
            security_group_ingress=[ec2.CfnSecurityGroup.IngressProperty(
                ip_protocol="tcp", from_port=22, to_port=22,
                source_security_group_id=config["endpoint_sg_id"])],
            security_group_egress=[ec2.CfnSecurityGroup.EgressProperty(
                ip_protocol="tcp", from_port=443, to_port=443, cidr_ip="0.0.0.0/0")],
            tags=tags + [CfnTag(key="Name", value=name_prefix + "security-" + b["run_id"])])
        pin = json.loads((ROOT.parent / "chromium-reference.json").read_text())["archives"]["linux64"]
        bootstrap = (ROOT / "bootstrap.sh").read_text()
        # Keep the exact wrapper within EC2's 16-KiB user-data limit. GzipFile
        # omits source names/time and uses the platform-independent OS header.
        wrapper = io.BytesIO()
        with gzip.GzipFile(fileobj=wrapper, mode="wb", filename="", mtime=0) as compressed:
            compressed.write((ROOT / "guest_capture.py").read_bytes())
        for name, value in {"RUN_ID": b["run_id"], "SSH_KEY_B64": base64.b64encode(key.encode()).decode(),
                            "CHROME_URL": pin["url"], "CHROME_SHA256": pin["sha256"],
                            "UBUNTU_SNAPSHOT": snapshot,
                            "CAPTURE_WRAPPER_GZIP_B64": base64.b64encode(wrapper.getvalue()).decode(),
                            "GUEST_STOP_MINUTES": str(runner["guest_stop_minutes"])}.items():
            bootstrap = bootstrap.replace("@@" + name + "@@", value)
        instance = ec2.CfnInstance(self, "Runner", image_id=image["id"], instance_type=runner["instance_type"],
            block_device_mappings=[ec2.CfnInstance.BlockDeviceMappingProperty(device_name="/dev/sda1",
                ebs=ec2.CfnInstance.EbsProperty(volume_size=runner["root_gib"], volume_type="gp3", iops=runner["gp3_iops"],
                    encrypted=True, kms_key_id="alias/aws/ebs", delete_on_termination=True))],
            network_interfaces=[ec2.CfnInstance.NetworkInterfaceProperty(device_index="0",
                description="Borrowser Chromium comparison capture " + b["run_id"],
                subnet_id=config["subnet_id"], group_set=[sg.attr_group_id, config["target_sg_id"]],
                associate_public_ip_address=True, delete_on_termination=True)],
            metadata_options=ec2.CfnInstance.MetadataOptionsProperty(http_tokens="required", http_put_response_hop_limit=1),
            user_data=Fn.base64(bootstrap),
            tags=tags + [CfnTag(key="Name", value=name_prefix + "runner-" + b["run_id"])],
            propagate_tags_to_volume_on_creation=True,
            disable_api_termination=False, monitoring=False, instance_initiated_shutdown_behavior="stop")
        for logical_id, resource in [("Runner", instance), ("RunnerSecurityGroup", sg)]:
            resource.override_logical_id(logical_id)
            resource.apply_removal_policy(RemovalPolicy.DESTROY)
