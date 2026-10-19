import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from aws_cdk import App
from shared_stack import SharedStack


def config():
    return {"account": "123456789012", "region": "eu-west-1", "vpc_cidr": "10.77.0.0/24",
            "subnet_cidr": "10.77.0.0/26", "availability_zone": "eu-west-1a",
            "ebs_key_arn": "arn:aws:kms:eu-west-1:123456789012:key/01234567-89ab-cdef-0123-456789abcdef"}


IMAGE = {"id": "ami-0123456789abcdef0"}


class SharedInfrastructure(unittest.TestCase):
    def synth(self, values=None, image=IMAGE):
        with tempfile.TemporaryDirectory() as directory:
            app = App(outdir=directory, analytics_reporting=False, auto_synth=False)
            stack = SharedStack(app, config() if values is None else values, image)
            assembly = app.synth()
            manifest = json.loads((Path(directory) / "manifest.json").read_text())
            self.assertFalse(any(a["type"] == "aws:cdk:asset-manifest"
                                 for a in manifest["artifacts"].values()))
            self.assertTrue(manifest["artifacts"][stack.artifact_id]["properties"]["terminationProtection"])
            return assembly.get_stack_by_name(stack.stack_name).template

    def test_shared_resource_boundary_and_routes(self):
        t = self.synth()
        self.assertEqual({name: r["Type"] for name, r in t["Resources"].items()}, {
            "Vpc": "AWS::EC2::VPC", "PublicSubnet": "AWS::EC2::Subnet",
            "InternetGateway": "AWS::EC2::InternetGateway", "GatewayAttachment": "AWS::EC2::VPCGatewayAttachment",
            "PublicRoutes": "AWS::EC2::RouteTable", "InternetRoute": "AWS::EC2::Route",
            "PublicRouteAssociation": "AWS::EC2::SubnetRouteTableAssociation",
            "CaptureExecutionRole": "AWS::IAM::Role", "InstanceConnectEndpoint": "AWS::EC2::InstanceConnectEndpoint",
            "EndpointSecurityGroup": "AWS::EC2::SecurityGroup", "RunnerTransportTargetSecurityGroup": "AWS::EC2::SecurityGroup"})
        self.assertNotIn("Parameters", t)
        self.assertNotIn("Rules", t)
        for resource in t["Resources"].values():
            self.assertEqual(resource["DeletionPolicy"], "Delete")
            self.assertEqual(resource["UpdateReplacePolicy"], "Delete")
        p = {name: r["Properties"] for name, r in t["Resources"].items()}
        names = {"Vpc": "vpc", "PublicSubnet": "public-eu-west-1a",
                 "InternetGateway": "internet-gateway", "PublicRoutes": "public-routes",
                 "CaptureExecutionRole": "capture-execution"}
        for logical_id, suffix in names.items():
            self.assertEqual({tag["Key"]: tag["Value"] for tag in p[logical_id]["Tags"]},
                {"Name": "borrowser-chromium-comparison-" + suffix,
                 "borrowser:purpose": "ag4-shared"})
        for logical_id in ("GatewayAttachment", "InternetRoute", "PublicRouteAssociation"):
            self.assertNotIn("Tags", p[logical_id])
        self.assertEqual(p["Vpc"]["CidrBlock"], config()["vpc_cidr"])
        self.assertTrue(p["Vpc"]["EnableDnsSupport"])
        self.assertTrue(p["Vpc"]["EnableDnsHostnames"])
        self.assertEqual(p["PublicSubnet"]["VpcId"], {"Ref": "Vpc"})
        self.assertEqual(p["PublicSubnet"]["AvailabilityZone"], "eu-west-1a")
        self.assertEqual(p["PublicSubnet"]["CidrBlock"], config()["subnet_cidr"])
        self.assertFalse(p["PublicSubnet"]["MapPublicIpOnLaunch"])
        self.assertEqual(p["GatewayAttachment"], {"VpcId": {"Ref": "Vpc"},
                                                "InternetGatewayId": {"Ref": "InternetGateway"}})
        self.assertEqual(p["PublicRoutes"]["VpcId"], {"Ref": "Vpc"})
        self.assertEqual(p["InternetRoute"], {"DestinationCidrBlock": "0.0.0.0/0",
            "GatewayId": {"Ref": "InternetGateway"}, "RouteTableId": {"Ref": "PublicRoutes"}})
        self.assertIn("GatewayAttachment", t["Resources"]["InternetRoute"]["DependsOn"])
        self.assertEqual(p["PublicRouteAssociation"], {"SubnetId": {"Ref": "PublicSubnet"},
                                                     "RouteTableId": {"Ref": "PublicRoutes"}})
        self.assertEqual(set(t["Outputs"]), {"VpcId", "SubnetId", "CloudFormationRoleArn"})
        self.assertEqual(t["Outputs"]["CloudFormationRoleArn"]["Value"],
                         {"Fn::GetAtt": ["CaptureExecutionRole", "Arn"]})
        self.assertTrue(all("Export" not in output for output in t["Outputs"].values()))
        self.assertLess(len(json.dumps(t).encode()), 51200)

    def test_exact_eice_rules_and_no_controller_iam(self):
        t = self.synth()
        p = {name: r["Properties"] for name, r in t["Resources"].items()}
        self.assertEqual(p["InstanceConnectEndpoint"], {
            "SubnetId": {"Ref": "PublicSubnet"}, "PreserveClientIp": False,
            "SecurityGroupIds": [{"Fn::GetAtt": ["EndpointSecurityGroup", "GroupId"]}],
            "Tags": [{"Key": "borrowser:purpose", "Value": "ag4-shared"}]})
        for name in ("EndpointSecurityGroup", "RunnerTransportTargetSecurityGroup"):
            self.assertEqual(p[name].get("SecurityGroupIngress", []), [])
            self.assertEqual(p[name]["VpcId"], {"Ref": "Vpc"})
        self.assertEqual(p["EndpointSecurityGroup"]["SecurityGroupEgress"], [{
            "IpProtocol": "tcp", "FromPort": 22, "ToPort": 22,
            "DestinationSecurityGroupId": {"Fn::GetAtt": ["RunnerTransportTargetSecurityGroup", "GroupId"]}}])
        self.assertEqual(p["RunnerTransportTargetSecurityGroup"]["SecurityGroupEgress"], [{
            "IpProtocol": "tcp", "FromPort": 443, "ToPort": 443, "CidrIp": "0.0.0.0/0"}])
        self.assertEqual([name for name, r in t["Resources"].items() if r["Type"].startswith("AWS::IAM::")], ["CaptureExecutionRole"])
        self.assertNotIn("OpenTunnel", json.dumps(t))

    def test_documented_external_controller_prerequisite(self):
        # This policy is reviewed account configuration, never CDK provisioning.
        contract = (ROOT.parents[2] / "docs/conformance/ag4-disposable-aws-chromium-runner.md").read_text()
        section = contract.split("### Externally administered controller IAM", 1)[1]
        policy = json.loads(section.split("```json\n", 1)[1].split("```", 1)[0])
        tunnel, observations, shared = policy["Statement"]
        self.assertEqual(tunnel["Action"], "ec2-instance-connect:OpenTunnel")
        self.assertEqual(tunnel["Resource"], "arn:aws:ec2:eu-west-1:123456789012:instance-connect-endpoint/eice-0123456789abcdef0")
        self.assertEqual(tunnel["Condition"], {
            "StringEquals": {"aws:RequestedRegion": "eu-west-1", "aws:ResourceTag/borrowser:purpose": "ag4-shared"},
            "NumericEquals": {"ec2-instance-connect:remotePort": 22},
            "NumericLessThanEquals": {"ec2-instance-connect:maxTunnelDuration": 300},
            "IpAddress": {"ec2-instance-connect:privateIpAddress": "10.77.0.0/26"}})
        self.assertEqual(set(observations["Action"]), {"ec2:DescribeInstanceConnectEndpoints", "ec2:DescribeSecurityGroups", "ec2:DescribeNetworkInterfaces"})
        self.assertEqual(observations["Resource"], "*")
        self.assertEqual(set(shared["Action"]), {"cloudformation:DescribeStacks", "cloudformation:GetTemplate", "cloudformation:ListStackResources"})
        self.assertNotIn("*", shared["Resource"])
        for statement in (observations, shared):
            self.assertEqual(statement["Condition"], {"StringEquals": {"aws:RequestedRegion": "eu-west-1"}})

    def test_service_role_is_distinct_and_limited_to_capture_resources(self):
        p = self.synth()["Resources"]["CaptureExecutionRole"]["Properties"]
        self.assertNotIn("ManagedPolicyArns", p)
        self.assertNotIn("RoleName", p)
        self.assertEqual(p["Path"], "/borrowser/")
        self.assertEqual(p["Description"],
            "CloudFormation execution role for disposable Borrowser Chromium comparison captures")
        self.assertEqual(p["AssumeRolePolicyDocument"], {"Version": "2012-10-17", "Statement": [{
            "Effect": "Allow", "Principal": {"Service": "cloudformation.amazonaws.com"},
            "Action": "sts:AssumeRole"}]})
        statements = {s["Sid"]: s for s in p["Policies"][0]["PolicyDocument"]["Statement"]}
        self.assertEqual(len(statements), 13)
        self.assertEqual(statements["LaunchWithSharedTransportTarget"], {
            "Sid": "LaunchWithSharedTransportTarget", "Effect": "Allow", "Action": "ec2:RunInstances",
            "Resource": {"Fn::Join": ["", ["arn:aws:ec2:eu-west-1:123456789012:security-group/",
                {"Fn::GetAtt": ["RunnerTransportTargetSecurityGroup", "GroupId"]}]]}})
        inputs = statements["LaunchPinnedInputs"]["Resource"]
        self.assertIn("arn:aws:ec2:eu-west-1::image/" + IMAGE["id"], inputs)
        self.assertIn({"Fn::Join": ["", ["arn:aws:ec2:eu-west-1:123456789012:subnet/",
                                        {"Ref": "PublicSubnet"}]]}, inputs)
        instance = statements["LaunchCaptureInstance"]
        self.assertEqual(instance["Condition"]["StringEquals"]["ec2:InstanceType"], "c6i.large")
        self.assertEqual(instance["Condition"]["StringEquals"]["aws:RequestTag/borrowser:purpose"], "ag4")
        root = statements["LaunchCaptureRoot"]
        self.assertEqual(root["Condition"]["NumericEquals"], {"ec2:VolumeSize": "16"})
        self.assertEqual(root["Condition"]["Bool"], {"ec2:Encrypted": "true"})
        self.assertEqual(root["Condition"]["StringEquals"]["ec2:VolumeType"], "gp3")
        for name in ("LaunchCaptureInstance", "LaunchCaptureRoot", "CreateCaptureGroup", "TagAtCreation"):
            self.assertEqual(statements[name]["Condition"]["Null"], {"aws:RequestTag/borrowser:run-id": "false"})
        for name in ("TerminateCaptureInstance", "ConfigureAndDeleteCaptureGroup", "LaunchWithCaptureGroup"):
            self.assertEqual(statements[name]["Condition"]["StringEquals"], {"ec2:ResourceTag/borrowser:purpose": "ag4"})
        for name in ("UseExistingEbsKey", "EbsServiceGrant"):
            self.assertEqual(statements[name]["Resource"], config()["ebs_key_arn"])
            self.assertEqual(statements[name]["Condition"]["StringEquals"], {
                "kms:ViaService": "ec2.eu-west-1.amazonaws.com", "kms:CallerAccount": config()["account"]})
        self.assertEqual(statements["EbsServiceGrant"]["Condition"]["Bool"], {"kms:GrantIsForAWSResource": "true"})
        for statement in statements.values():
            actions = statement["Action"]
            if isinstance(actions, str): actions = [actions]
            self.assertFalse(any("*" in a or a.startswith(("iam:", "s3:", "cloudformation:")) for a in actions))
            self.assertFalse(any(a in actions for a in ("ec2:CreateVolume", "ec2:DeleteVolume",
                "ec2:CreateVpc", "ec2:DeleteVpc", "kms:CreateKey", "kms:CreateAlias")))
            if statement["Resource"] == "*":
                self.assertTrue(all(a.startswith("ec2:Describe") for a in actions))

    def test_rejects_unreviewed_or_cross_account_configuration(self):
        cases = [("region", "eu-central-1"), ("account", "default"), ("availability_zone", "us-east-1a"),
                 ("vpc_cidr", "0.0.0.0/0"), ("vpc_cidr", "192.0.2.0/24"),
                 ("subnet_cidr", "10.78.0.0/26"), ("subnet_cidr", "2001:db8::/64"),
                 ("ebs_key_arn", "alias/aws/ebs"), ("ebs_key_arn", config()["ebs_key_arn"].replace("123456789012", "123456789013")),
                 ("credentials", "not-an-accepted-input")]
        for key, value in cases:
            with self.subTest(key=key, value=value):
                c = copy.deepcopy(config()); c[key] = value
                with self.assertRaises(ValueError): self.synth(c)
        for image in (None, {}, {"id": "latest"}):
            with self.subTest(image=image):
                with self.assertRaises(ValueError): self.synth(image=image)

    def test_actual_entry_point_uses_repository_pin_without_aws_credentials(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("shared_app.py", "shared_stack.py"):
                shutil.copyfile(ROOT / name, root / name)
            (root / "runner.json").write_text(json.dumps({"image": IMAGE}))
            (root / "input.json").write_text(json.dumps(config()))
            env = {k: v for k, v in os.environ.items() if not k.startswith(("AWS_", "BOTO"))}
            env.update(HOME=str(root), AWS_CONFIG_FILE="/dev/null", AWS_SHARED_CREDENTIALS_FILE="/dev/null",
                       AWS_EC2_METADATA_DISABLED="true", PYTHONDONTWRITEBYTECODE="1")
            command = [sys.executable, str(root / "shared_app.py"), "--input", str(root / "input.json"),
                       "--out", str(root / "cdk.out")]
            result = subprocess.run(command, env=env, capture_output=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            template = json.loads(result.stdout)
            self.assertEqual(len(template["Resources"]), 11)
            self.assertIn(IMAGE["id"], result.stdout.decode())
            # Unset production pins must fail instead of using a default/latest AMI.
            (root / "runner.json").write_text(json.dumps({"image": None}))
            result = subprocess.run(command, env=env, capture_output=True, timeout=60)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b"")
            self.assertIn(b"reviewed runner.json AMI pin required", result.stderr)


if __name__ == "__main__":
    unittest.main()
