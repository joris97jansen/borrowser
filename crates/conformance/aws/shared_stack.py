"""Explicit shared AG4 prerequisites; never created or deleted by a capture."""
import ipaddress
import re

from aws_cdk import CfnOutput, CfnTag, Environment, LegacyStackSynthesizer, RemovalPolicy, Stack
from aws_cdk import aws_ec2 as ec2, aws_iam as iam


class SharedStack(Stack):
    def __init__(self, app, config, reviewed_image):
        required = {"account", "region", "vpc_cidr", "subnet_cidr", "availability_zone", "ebs_key_arn"}
        if set(config) != required or any(not isinstance(v, str) for v in config.values()):
            raise ValueError("exact nonsecret shared configuration required")
        account = config["account"]
        region = config["region"]
        if not re.fullmatch(r"\d{12}", account) or region != "eu-west-1":
            raise ValueError("one explicit account in eu-west-1 required")
        if not re.fullmatch(r"eu-west-1[a-z]", config["availability_zone"]):
            raise ValueError("explicit eu-west-1 availability zone required")
        vpc_net = ipaddress.ip_network(config["vpc_cidr"])
        subnet_net = ipaddress.ip_network(config["subnet_cidr"])
        private_ranges = [ipaddress.ip_network(n) for n in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16")]
        if (vpc_net.version != 4 or subnet_net.version != 4
                or not 16 <= vpc_net.prefixlen <= subnet_net.prefixlen <= 28
                or not any(vpc_net.subnet_of(n) for n in private_ranges)
                or not subnet_net.subnet_of(vpc_net)):
            raise ValueError("explicit RFC1918 VPC and contained IPv4 subnet required")
        if not isinstance(reviewed_image, dict) or not re.fullmatch(
                r"ami-[0-9a-f]{17}", reviewed_image.get("id", "")):
            raise ValueError("reviewed runner.json AMI pin required")
        key_arn = config["ebs_key_arn"]
        if not re.fullmatch(
                rf"arn:aws:kms:{region}:{account}:key/[0-9a-f]{{8}}(?:-[0-9a-f]{{4}}){{3}}-[0-9a-f]{{12}}", key_arn):
            raise ValueError("existing regional alias/aws/ebs target key ARN required")

        super().__init__(app, "borrowser-ag4-shared", stack_name="borrowser-ag4-shared",
                         env=Environment(account=account, region=region),
                         synthesizer=LegacyStackSynthesizer(), analytics_reporting=False,
                         termination_protection=True)
        tags = [CfnTag(key="borrowser:purpose", value="ag4-shared")]
        vpc = ec2.CfnVPC(self, "Vpc", cidr_block=str(vpc_net), enable_dns_support=True,
                        enable_dns_hostnames=True, instance_tenancy="default", tags=tags)
        subnet = ec2.CfnSubnet(self, "PublicSubnet", vpc_id=vpc.ref, cidr_block=str(subnet_net),
                              availability_zone=config["availability_zone"],
                              map_public_ip_on_launch=False, tags=tags)
        gateway = ec2.CfnInternetGateway(self, "InternetGateway", tags=tags)
        attachment = ec2.CfnVPCGatewayAttachment(self, "GatewayAttachment", vpc_id=vpc.ref,
                                                internet_gateway_id=gateway.ref)
        routes = ec2.CfnRouteTable(self, "PublicRoutes", vpc_id=vpc.ref, tags=tags)
        route = ec2.CfnRoute(self, "InternetRoute", route_table_id=routes.ref,
                            destination_cidr_block="0.0.0.0/0", gateway_id=gateway.ref)
        route.add_resource_dependency(attachment)
        association = ec2.CfnSubnetRouteTableAssociation(self, "PublicRouteAssociation",
                                                        subnet_id=subnet.ref, route_table_id=routes.ref)

        target_group = ec2.CfnSecurityGroup(self, "RunnerTransportTargetSecurityGroup",
            vpc_id=vpc.ref, group_description="Shared Borrowser Chromium runner transport target",
            security_group_ingress=[], security_group_egress=[ec2.CfnSecurityGroup.EgressProperty(
                ip_protocol="tcp", from_port=443, to_port=443, cidr_ip="0.0.0.0/0")], tags=tags)
        endpoint_group = ec2.CfnSecurityGroup(self, "EndpointSecurityGroup",
            vpc_id=vpc.ref, group_description="Shared Borrowser Chromium EICE transport",
            security_group_ingress=[], security_group_egress=[ec2.CfnSecurityGroup.EgressProperty(
                ip_protocol="tcp", from_port=22, to_port=22,
                destination_security_group_id=target_group.attr_group_id)], tags=tags)
        endpoint = ec2.CfnInstanceConnectEndpoint(self, "InstanceConnectEndpoint",
            subnet_id=subnet.ref, security_group_ids=[endpoint_group.attr_group_id],
            preserve_client_ip=False, tags=tags)
        # This pinned CloudFormation resource exposes no IpAddressType input.
        # The subnet is IPv4-only; live preflight additionally requires ipv4.

        # These statements serve only the existing two-resource capture template.
        # They are not a policy evaluator or permissions for shared-stack deployment.
        prefix = f"arn:aws:ec2:{region}:{account}:"
        vpc_arn = prefix + "vpc/" + vpc.ref
        requested_tags = {"aws:RequestTag/borrowser:purpose": "ag4"}
        run_tag = {"aws:RequestTag/borrowser:run-id": "false"}
        owned = {"StringEquals": {"ec2:ResourceTag/borrowser:purpose": "ag4"}}
        via_ebs = {"kms:ViaService": f"ec2.{region}.amazonaws.com", "kms:CallerAccount": account}
        statements = [
            {"Sid": "ObserveCaptureResources", "Effect": "Allow", "Action": [
                "ec2:DescribeInstances", "ec2:DescribeInstanceAttribute", "ec2:DescribeImages",
                "ec2:DescribeVolumes", "ec2:DescribeNetworkInterfaces", "ec2:DescribeSecurityGroups",
                "ec2:DescribeSubnets", "ec2:DescribeVpcs", "ec2:DescribeTags"], "Resource": "*",
             "Condition": {"StringEquals": {"aws:RequestedRegion": region}}},
            {"Sid": "LaunchPinnedInputs", "Effect": "Allow", "Action": "ec2:RunInstances",
             "Resource": [f"arn:aws:ec2:{region}::image/" + reviewed_image["id"],
                          prefix + "subnet/" + subnet.ref, prefix + "network-interface/*"]},
            {"Sid": "LaunchWithCaptureGroup", "Effect": "Allow", "Action": "ec2:RunInstances",
             "Resource": prefix + "security-group/*", "Condition": owned},
            {"Sid": "LaunchCaptureInstance", "Effect": "Allow", "Action": "ec2:RunInstances",
             "Resource": prefix + "instance/*", "Condition": {
                 "StringEquals": {**requested_tags, "ec2:InstanceType": "c6i.large"}, "Null": run_tag}},
            {"Sid": "LaunchCaptureRoot", "Effect": "Allow", "Action": "ec2:RunInstances",
             "Resource": prefix + "volume/*", "Condition": {
                 "StringEquals": {**requested_tags, "ec2:VolumeType": "gp3"},
                 "NumericEquals": {"ec2:VolumeSize": "16"}, "Bool": {"ec2:Encrypted": "true"},
                 "Null": run_tag}},
            {"Sid": "CreateGroupInSharedVpc", "Effect": "Allow", "Action": "ec2:CreateSecurityGroup",
             "Resource": vpc_arn},
            {"Sid": "CreateCaptureGroup", "Effect": "Allow", "Action": "ec2:CreateSecurityGroup",
             "Resource": prefix + "security-group/*", "Condition": {
                 "StringEquals": requested_tags, "Null": run_tag}},
            {"Sid": "TagAtCreation", "Effect": "Allow", "Action": "ec2:CreateTags",
             "Resource": [prefix + kind + "/*" for kind in ("instance", "volume", "security-group")],
             "Condition": {"StringEquals": {**requested_tags,
                 "ec2:CreateAction": ["RunInstances", "CreateSecurityGroup"]}, "Null": run_tag}},
            {"Sid": "ConfigureAndDeleteCaptureGroup", "Effect": "Allow", "Action": [
                "ec2:AuthorizeSecurityGroupIngress", "ec2:AuthorizeSecurityGroupEgress",
                "ec2:RevokeSecurityGroupIngress", "ec2:RevokeSecurityGroupEgress",
                "ec2:DeleteSecurityGroup"], "Resource": prefix + "security-group/*", "Condition": owned},
            {"Sid": "TerminateCaptureInstance", "Effect": "Allow", "Action": "ec2:TerminateInstances",
             "Resource": prefix + "instance/*", "Condition": owned},
            {"Sid": "UseExistingEbsKey", "Effect": "Allow", "Action": ["kms:Decrypt", "kms:DescribeKey",
                "kms:GenerateDataKeyWithoutPlaintext", "kms:ReEncryptFrom", "kms:ReEncryptTo"],
             "Resource": key_arn, "Condition": {"StringEquals": via_ebs}},
            {"Sid": "EbsServiceGrant", "Effect": "Allow", "Action": "kms:CreateGrant", "Resource": key_arn,
             "Condition": {"StringEquals": via_ebs, "Bool": {"kms:GrantIsForAWSResource": "true"}}},
        ]
        # Only this exact shared SG may be attached in addition to the run-owned
        # SG. This is not permission to modify/delete either shared group.
        statements.append({"Sid": "LaunchWithSharedTransportTarget", "Effect": "Allow",
            "Action": "ec2:RunInstances",
            "Resource": prefix + "security-group/" + target_group.attr_group_id})
        role = iam.CfnRole(self, "CaptureExecutionRole", path="/borrowser/",
            description="CloudFormation execution role for disposable Borrowser Chromium comparison captures",
            assume_role_policy_document={"Version": "2012-10-17", "Statement": [{
                "Effect": "Allow", "Principal": {"Service": "cloudformation.amazonaws.com"},
                "Action": "sts:AssumeRole"}]},
            policies=[iam.CfnRole.PolicyProperty(policy_name="ag4-capture", policy_document={
                "Version": "2012-10-17", "Statement": statements})], tags=tags)
        # Display names describe the harness; ownership and physical identities
        # remain independent of these mutable labels.
        for resource, suffix in (
                (vpc, "vpc"), (subnet, "public-" + config["availability_zone"]),
                (gateway, "internet-gateway"), (routes, "public-routes"),
                (role, "capture-execution")):
            resource.tags.set_tag("Name", "borrowser-chromium-comparison-" + suffix)
        for resource in (vpc, subnet, gateway, attachment, routes, route, association, role,
                         target_group, endpoint_group, endpoint):
            resource.override_logical_id(resource.node.id)
            resource.apply_removal_policy(RemovalPolicy.DESTROY)
        CfnOutput(self, "VpcId", value=vpc.ref)
        CfnOutput(self, "SubnetId", value=subnet.ref)
        CfnOutput(self, "CloudFormationRoleArn", value=role.attr_arn)
