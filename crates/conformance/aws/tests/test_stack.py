import base64
import gzip
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from aws_cdk import App
from stack import RunnerStack


def request():
    run = "0123456789abcdef0123456789abcdef"
    return {"binding": {"run_id": run, "account": "123456789012", "region": "eu-west-1",
        "stack_name": "borrowser-ag4-" + run, "role_arn": "arn:aws:iam::123456789012:role/ag4",
        "tags": {"borrowser:purpose": "ag4", "borrowser:run-id": run}, "timeout_minutes": 10,
        "disable_rollback": False, "termination_protection": False},
        "network": {"vpc_id": "vpc-0123456789abcdef0", "subnet_id": "subnet-0123456789abcdef0",
                    "endpoint_sg_id": "sg-11111111111111111", "target_sg_id": "sg-22222222222222222"},
        "reviewed_image": {"id": "ami-0123456789abcdef0"}, "ubuntu_snapshot": "20261001T000000Z",
        "ssh_public_key": "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEcm83UIknGkWMqM4JRfllStJlXXRhBszo9bo1ufsoTY"}


class Infrastructure(unittest.TestCase):
    def synth(self, config=None):
        with tempfile.TemporaryDirectory() as directory:
            app = App(outdir=directory, analytics_reporting=False, auto_synth=False)
            stack = RunnerStack(app, config or request())
            assembly = app.synth()
            template = assembly.get_stack_by_name(stack.stack_name).template
            manifest = json.loads((Path(directory) / "manifest.json").read_text())
            self.assertFalse(any(a["type"] == "aws:cdk:asset-manifest" for a in manifest["artifacts"].values()))
            return template

    def test_exact_resources_and_deletion_contract(self):
        t = self.synth()
        self.assertEqual(set(t["Resources"]), {"Runner", "RunnerSecurityGroup"})
        self.assertNotIn("Parameters", t)
        self.assertNotIn("Transform", t)
        self.assertEqual(t["Metadata"]["BorrowserAg4Operation"], request()["binding"])
        for r in t["Resources"].values():
            self.assertEqual(r["DeletionPolicy"], "Delete")
            self.assertEqual(r["UpdateReplacePolicy"], "Delete")
        p = t["Resources"]["Runner"]["Properties"]
        self.assertEqual(p["ImageId"], request()["reviewed_image"]["id"])
        self.assertEqual(p["InstanceType"], "c6i.large")
        self.assertEqual(p["BlockDeviceMappings"], [{"DeviceName": "/dev/sda1", "Ebs": {
            "DeleteOnTermination": True, "Encrypted": True, "Iops": 3000, "KmsKeyId": "alias/aws/ebs",
            "VolumeSize": 16, "VolumeType": "gp3"}}])
        self.assertEqual(len(p["NetworkInterfaces"]), 1)
        self.assertEqual(p["NetworkInterfaces"][0]["GroupSet"], [{"Fn::GetAtt": ["RunnerSecurityGroup", "GroupId"]}, "sg-22222222222222222"])
        self.assertTrue(p["NetworkInterfaces"][0]["DeleteOnTermination"])
        self.assertTrue(p["NetworkInterfaces"][0]["AssociatePublicIpAddress"])
        self.assertEqual(p["NetworkInterfaces"][0]["SubnetId"], request()["network"]["subnet_id"])
        self.assertTrue(p["PropagateTagsToVolumeOnCreation"])
        self.assertIn({"Key": "borrowser:stack-id", "Value": {"Ref": "AWS::StackId"}}, p["Tags"])
        self.assertIn({"Key": "Name", "Value": "borrowser-chromium-comparison-runner-" + request()["binding"]["run_id"]}, p["Tags"])
        self.assertEqual(p["NetworkInterfaces"][0]["Description"],
                         "Borrowser Chromium comparison capture " + request()["binding"]["run_id"])
        self.assertEqual(p["MetadataOptions"]["HttpTokens"], "required")
        self.assertNotIn("IamInstanceProfile", p)
        self.assertNotIn("KeyName", p)
        self.assertFalse(p["DisableApiTermination"])
        self.assertEqual(p["InstanceInitiatedShutdownBehavior"], "stop")
        sg = t["Resources"]["RunnerSecurityGroup"]["Properties"]
        self.assertIn({"Key": "Name", "Value": "borrowser-chromium-comparison-security-" + request()["binding"]["run_id"]}, sg["Tags"])
        self.assertEqual(sg["GroupDescription"], "Disposable Borrowser Chromium comparison capture")
        self.assertEqual(sg["SecurityGroupIngress"], [{"IpProtocol": "tcp", "FromPort": 22, "ToPort": 22, "SourceSecurityGroupId": "sg-11111111111111111"}])
        self.assertEqual(sg["SecurityGroupEgress"], [{"IpProtocol": "tcp", "FromPort": 443, "ToPort": 443, "CidrIp": "0.0.0.0/0"}])
        bootstrap = p["UserData"]["Fn::Base64"]
        wrapper = (Path(__file__).resolve().parents[1] / "guest_capture.py").read_bytes()
        encoded = re.search(r"printf '%s' '([^']+)' \| base64 --decode \| gzip --decompress", bootstrap)[1]
        compressed = base64.b64decode(encoded, validate=True)
        self.assertEqual(gzip.decompress(compressed), wrapper)
        self.assertEqual(compressed[:10], b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\xff")
        self.assertIn("python3 iproute2 gzip", bootstrap)
        self.assertIn("/opt/borrowser-ag4/guest_capture.py", bootstrap)
        self.assertLessEqual(len(bootstrap.encode()), 16384)
        self.assertEqual(re.findall(r"^run_phase (\w+) (\d+)$", bootstrap, re.M), [
            ("setup", "30"), ("apt_update", "180"), ("apt_install", "420"),
            ("chromium_download", "180"), ("archive", "45"), ("sandbox", "15"),
            ("ssh_prepare", "10"), ("ssh_config", "5"), ("ssh_validate", "5"),
            ("ssh_restart", "10"), ("ssh_probe_loopback", "5"), ("ssh_probe_private", "10"), ("ready", "5")])
        self.assertIn('timeout --kill-after=5 "$seconds"', bootstrap)
        self.assertIn("BORROWSER_AG4_BOOTSTRAP_FAILED " + request()["binding"]["run_id"] + " %s %d", bootstrap)
        for required in ["--on-active=45m", "sha256sum --check --strict", "chmod -R a+rX /opt/borrowser-ag2", "apparmor_parser", "userns,", "BORROWSER_AG4_READY " + request()["binding"]["run_id"]]:
            self.assertIn(required, bootstrap)
        for forbidden in ["--no-sandbox", "@@", "aws ec2", "aws s3"]:
            self.assertNotIn(forbidden, bootstrap)
        self.assertLessEqual(len(json.dumps(t).encode()), 51200)
        self.assertEqual(t, json.loads((Path(__file__).parent / "synthesized.json").read_text()))

    def test_rejects_unbounded_network_and_unpinned_image(self):
        for field, value in [("controller_cidr", "192.0.2.1/32"), ("endpoint_sg_id", "sg-*"), ("target_sg_id", "sg-11111111111111111")]:
            r = request(); r["network"][field] = value
            with self.assertRaises(ValueError): self.synth(r)
        r = request(); r["reviewed_image"]["id"] = "latest"
        with self.assertRaises(ValueError): self.synth(r)


if __name__ == "__main__":
    unittest.main()
