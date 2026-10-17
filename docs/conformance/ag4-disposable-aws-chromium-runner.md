# AG4 disposable AWS Chromium runner

AG4 belongs to conformance tooling. AWS supplies one disposable Linux execution
environment for the existing AG2 capture. Validated capture data enters the
existing AG3 comparison projection after the entire remote phase and cleanup.
No CSS, layout, paint, browser, fixture, expectation, capture-schema, or
comparison-schema changes are part of this runner.

## Qualification status

The current implementation carries ordinary SSH through one shared EC2 Instance
Connect Endpoint (EICE). This is still **AG4**, within AG's existing nine issues.
Implementation and local qualification do not authorize AWS execution or close
AG4. The eleven-resource shared update, corporate Zscaler/EICE TLS path, first
normal `canvas/cascade` run and subsequent broader acceptance remain pending.
Earlier live operations qualified guest readiness and CloudFormation cleanup;
their direct-public SSH attempts failed during banner exchange before host-key
exchange, authentication, upload or Chromium execution. That evidence does not
qualify the new transport.

`crates/conformance/aws/runner.json` pins the reviewed AMI identity, source root
size and Ubuntu package snapshot below. Synthetic test IDs are never production
defaults. Live creation is still refused if either pin is missing. Pin review
does not establish bootstrap, sandbox, deployment-role or AWS capture/cleanup
qualification; that real acceptance evidence remains outstanding. Local AG2/AG3
Linux qualification does not qualify this AWS environment.

On 2026-10-14, read-only SSO observations in `eu-west-1` verified candidate
`ami-00bf3d24573e7276a` as available, Canonical-owned, x86-64/HVM/EBS-backed,
named `ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-20260923`.
Its completed Canonical root snapshot is `snap-0f8dbea1d46057399`, 8 GiB,
recorded as `image.source_root_gib`. Its image mapping also contains
`/dev/sdb: ephemeral0` and `/dev/sdc: ephemeral1`. The initial preflight rejected
these declarations because it incorrectly required one mapping object. The
corrected preflight distinguishes declarations from effective storage:

- Exactly one EBS root is required, found by `/dev/sda1` rather than array
  position, with the exact snapshot and reviewed source size (1–16 GiB).
- Only the two exact ancillary device/virtual-name pairs above are permitted,
  optionally absent and in any order. Additional EBS mappings, mixed kinds,
  duplicate devices/virtual identities, unknown devices, malformed or extra
  ancillary fields, and missing roots are rejected.
- This allowance is tied to `eu-west-1`, x86-64 and the fixed `c6i.large`, which
  was observed with `InstanceStorageSupported=false`. It does not enable a
  different instance type or a runtime capability-discovery mechanism.
- Post-launch observations still require exactly one actual EBS mapping and
  delete-on-termination. Extra launched mappings remain failures. CloudFormation
  still explicitly creates an **encrypted 16-GiB gp3 root** using `alias/aws/ebs`;
  the unencrypted source snapshot does not permit an unencrypted guest disk.
  Root-volume identity and terminal cleanup checks are unchanged.

AWS documents the distinction between [EBS and instance-store mappings](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/block-device-mapping-concepts.html)
and the [instance type's storage limits](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/instance-store-volumes.html).
These are reviewed repository pins, not per-run latest-image lookups or operator
image overrides. The pinned package snapshot `20261013T000000Z` has
published indexes containing the explicit bootstrap packages; this establishes
availability, not successful package installation or guest qualification.

## Infrastructure and shared prerequisites

`crates/conformance/aws/app.py` and `stack.py` are the sole capture infrastructure
definition; the separate shared definition is described below. Python CDK uses `LegacyStackSynthesizer`, no assets, no context
lookups, and no bootstrap version rule. Rust runs synthesis and submits the
resulting JSON through CloudFormation `CreateStack` with `TemplateBody`.
No CDK toolkit stack, asset bucket, ECR repository, S3 upload, or custom resource
is needed. Transitive CDK asset *packages* in the dependency lock do not create
infrastructure; synthesis tests assert the absence of asset manifests.

Each operation creates exactly:

| Template resource | Configuration and deletion |
| --- | --- |
| `Runner`, `AWS::EC2::Instance` | One on-demand `c6i.large`, 2 vCPU/4 GiB; one encrypted 16-GiB gp3 root disk, 3,000 IOPS; one primary ENI and automatic public IPv4; IMDSv2; no instance profile, EC2 key pair, detailed monitoring, EIP, or additional disks. |
| `RunnerSecurityGroup`, `AWS::EC2::SecurityGroup` | Inline TCP/22 ingress only from the verified shared `EndpointSecurityGroup`; explicit outbound TCP/443 to `0.0.0.0/0` for pinned downloads. |

Both resources have `DeletionPolicy: Delete` and `UpdateReplacePolicy: Delete`.
Root storage and the primary ENI have `DeleteOnTermination=true`. Root-volume
tags propagate at creation and include the run ID and the unique stack ARN.
CloudFormation's inline instance EBS mapping has no throughput property, so the
runner uses gp3's included 125-MiB/s default; it does not provision additional
throughput. The existing AWS-managed `alias/aws/ebs` key is used; AG4 creates no
KMS resources. Termination protection is disabled. Guest shutdown stops the
instance, preserving CloudFormation as the deletion owner.

The operator supplies or explicitly deploys these **shared**, non-run-owned prerequisites:

- An AWS account and pre-authenticated environment credentials for the pinned AWS CLI, with adequate session
  lifetime for deployment, capture, and the separate cleanup budget.
- An existing VPC and public subnet in `eu-west-1`, Internet gateway/routing,
  working VPC DNS, public IPv4 capacity, and outbound HTTPS connectivity.
- A reviewed shared EICE and its two security groups, with authenticated
  controller access to its AWS WebSocket/TLS service. Stable public controller
  source-IP identity is no longer a transport prerequisite.
- An existing CloudFormation execution role, trusted by CloudFormation, able
  to create/tag/delete the two declared resources and their implicit root disk
  and ENI, and to use the disclosed EBS encryption prerequisite.
- Controller permissions for STS `GetCallerIdentity`; CloudFormation
  `CreateStack`, `DescribeStacks`, `GetTemplate`, `ListStackResources`,
  `DescribeStackEvents`, and `DeleteStack`; `iam:PassRole` for exactly the
  execution role; targeted EC2 `DescribeImages`, `DescribeSubnets`,
  `DescribeInstances`, `DescribeVolumes`, and `GetConsoleOutput`, plus the
  narrowly scoped EICE/read prerequisite described below.

Scope write permissions to the AG4 stack names, tags, role and resources where
AWS supports it. Some EC2 describe operations require IAM resource `*`; the
runner still supplies operation-specific filters. It performs no account-wide
inventory, IAM policy evaluation, route/NACL compliance scan, or direct EC2
mutation. Shared networking, IAM and the EBS key are not deleted by a run.

### Explicit shared prerequisite stack

AG4 also provides `aws/shared_app.py` and `shared_stack.py`, following the
explicit scope extension to provision shared prerequisites in Python CDK.
This is a separate, operator-deployed `borrowser-ag4-shared` CloudFormation
stack. Capture and cleanup-only never synthesize, create, update or delete it.
The disposable capture template still has exactly its original two resources.

The shared template contains exactly eleven resources:

| Resource | Purpose |
| --- | --- |
| `Vpc` | One explicitly addressed RFC1918 IPv4 VPC with DNS support/hostnames. |
| `PublicSubnet` | One explicitly addressed subnet and availability zone; automatic public-IP assignment is disabled because the capture template explicitly requests its IP. |
| `InternetGateway`, `GatewayAttachment` | Internet access for the subnet. |
| `PublicRoutes`, `InternetRoute`, `PublicRouteAssociation` | Explicit subnet route table and IPv4 default route through that gateway. |
| `CaptureExecutionRole` | CloudFormation service role under `/borrowser/`, with an inline policy for the existing capture resources. |
| `InstanceConnectEndpoint` | Shared EICE in the reviewed IPv4 subnet, explicitly `PreserveClientIp=false`. |
| `EndpointSecurityGroup` | No ingress; only TCP/22 egress referencing `RunnerTransportTargetSecurityGroup`. |
| `RunnerTransportTargetSecurityGroup` | No ingress; explicit TCP/443 egress to `0.0.0.0/0`, matching the runner's required outbound policy. |

EC2 additionally supplies the VPC's standard default security group, network
ACL and main route table. These implicit shared objects are not used as the
capture security group or its explicit subnet route table; VPC deletion removes
them. There are no NAT gateways, additional VPC endpoints, EIPs, hosts, disks, buckets, custom
resources, CDK bootstrap assets or newly defined KMS keys in this stack.

Managed resource display names use `borrowser-chromium-comparison`, independently
of the issue number. The shared VPC, public subnet, gateway, route table and role
have `Name` tags ending in `vpc`, `public-<AZ>`, `internet-gateway`, `public-routes`
and `capture-execution`. The role also has a descriptive purpose statement.
Future capture instances and security groups use `runner-<run-id>` and
`security-<run-id>` suffixes. The root volume inherits the instance's name through
the existing tag propagation; the inline ENI has a capture/run description.

These labels are not deletion authority. Existing ownership tags, logical IDs,
stack identities and the generated IAM role name remain unchanged. Routes,
attachments and associations have no supported `Name` field in these resources;
implicit VPC defaults and the AWS-managed EBS key retain AWS-managed identities.
Naming them does not justify extra resources, custom resources or direct AWS
tagging outside CloudFormation. See the supported
[inline network-interface description](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-properties-ec2-instance-networkinterface.html)
and [IAM role tags/description](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-iam-role.html).

All eleven resources specify `DeletionPolicy: Delete` and
`UpdateReplacePolicy: Delete`; the stack is explicitly deployed with termination
protection. It persists between captures. Its lifecycle and any future charges
are separate from the per-operation admission estimate and cleanup guarantee.
Do not update/replace or remove shared resources while captures or recovery
records depend on them. Removal requires an explicit operator decision, verified
stack identity, disabling its termination protection and ordinary CloudFormation
deletion with terminal confirmation. Run cleanup never grants that authority.

The service role trusts CloudFormation, not the SSO caller. It restricts launches
to the repository AMI pin, generated subnet, `c6i.large` and encrypted 16-GiB gp3
root storage; creation requires the capture-purpose and run-ID tags. It allows
security-group creation in this VPC, tagged capture-group rules/deletion and
tagged instance termination. EC2 describes require `Resource: *` and are limited
to `eu-west-1`. EBS key use is limited to the explicitly verified existing key
ARN via regional EC2; grants require `kms:GrantIsForAWSResource`. No IAM, network
creation, direct volume deletion or KMS-key creation permission belongs to that
role. Synthesis assertions are not evidence that actual CloudFormation calls
will be authorized; live deployment and rollback remain qualification work.

The only EICE execution-role delta is the appended
`LaunchWithSharedTransportTarget` statement: `ec2:RunInstances` on
`arn:aws:ec2:eu-west-1:<account>:security-group/<derived-target-group-id>`, using
the target group's `Fn::GetAtt` in CDK. The previous twelve statements, role
identity, trust, path and other properties remain unchanged. This authorizes
attachment of that shared SG; it grants no authority to mutate it. Controller
IAM is an externally administered prerequisite, never a resource in this stack.

The runner primary ENI attaches exactly its run-owned SG and the shared target
SG. SG references match an attached group, not that group's rule contents; the
target SG therefore needs no ingress. Incoming TCP/22 is allowed by the run SG's
reference to the endpoint SG. SG rules combine across the ENI; both attached
groups permit only the same explicit outbound HTTPS rule. Stateful SSH replies
need no added outbound rule. No group relies on CloudFormation's implicit
allow-all egress. See AWS's [EICE SG contract](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/eice-security-groups.html).
The pinned CDK/CloudFormation resource has no `IpAddressType` property: synthesis
uses the existing IPv4-only subnet, and preflight requires the observed EICE
`IpAddressType` to be exactly `ipv4`.

Use the existing Python/Node pins and hash-locked dependencies. Put this exact
nonsecret input shape outside the repository (the values below are examples,
not reviewed operator configuration):

```json
{
  "account": "123456789012",
  "region": "eu-west-1",
  "vpc_cidr": "10.77.0.0/24",
  "subnet_cidr": "10.77.0.0/26",
  "availability_zone": "eu-west-1a",
  "ebs_key_arn": "arn:aws:kms:eu-west-1:123456789012:key/01234567-89ab-cdef-0123-456789abcdef"
}
```

Review the CIDRs/AZ for the intended account; there is no default-VPC discovery
or automatic network selection. The app takes its AMI from `runner.json` and
refuses an unset pin. It does not have an independent image override. Verify
that `ebs_key_arn` is the existing target of regional `alias/aws/ebs`, enabled
and AWS-managed; an arbitrary customer key is not the supported prerequisite.
The key ARN is a permission input, not a second volume-encryption definition.

An unbound predefined `alias/aws/ebs` is **not** an existing encryption key.
The 2026-10-14 observation found no `TargetKeyId` for that alias in the intended
account/region. AWS documents that `DescribeKey` on such an alias initializes
the AWS-managed key. That is a mutation and requires separate authorization;
it is never part of read-only preparation or credential-free synthesis. AG4
does not introduce a custom resource to initialize it. After authorized setup,
record and verify its literal key ARN before synthesizing the shared stack.
See [DescribeKey](https://docs.aws.amazon.com/kms/latest/APIReference/API_DescribeKey.html)
and [EBS encryption permissions](https://docs.aws.amazon.com/ebs/latest/userguide/ebs-encryption-requirements.html).

### Shared authority and operator migration

The existing operator schema changes by removing `controller_cidr` and adding
only `shared_stack_id` and `shared_template_sha256`. See
[`operator.example.json`](../../crates/conformance/aws/operator.example.json).
Unknown/obsolete fields fail, including `controller_cidr`, a manual physical-ID
inventory, endpoint IDs/DNS and runtime executable-identity fields. Account,
region (`eu-west-1`), VPC/subnet and execution-role expectations remain reviewed
operator inputs. The shared digest is SHA-256 of
`borrowser-ag4-template-v1\n` followed by canonical compact, sorted-key JSON;
it is not the raw-file SHA-256.

Preflight uses the exact shared StackId for `DescribeStacks`, `GetTemplate
Original` and `ListStackResources`. It requires successful terminal status,
termination protection, the approved digest and exactly the eleven logical IDs
and types above. It derives all physical IDs from those responses, cross-checks
the existing VPC/subnet/role inputs, then makes exact-ID subnet, endpoint, SG and
ENI observations. Missing, duplicated, extra, conflicting, malformed or
paginated observations fail. There is no account-wide discovery or substitute
endpoint. `VerifiedEndpoint` contains the authenticated DNS returned for that
exact endpoint, its account/region/VPC/subnet/AZ, IPv4 behavior, usable terminal
state, ownership tags and exact effective SG rules. It also retains the derived
resource map in memory for the post-cleanup equality check; the operator does
not duplicate that map.

After READY, `VerifiedTarget` binds the run ID and exact run StackId to its
`Runner` instance, primary ENI/device 0, primary private IPv4, reviewed network,
exact two-SG set, instance/run-SG ownership tags and console-derived guest key.
The ENI is verified through its attachment, address and SGs; no nonexistent ENI
tag contract is assumed. Public IPv4 is never an SSH target.

### Externally administered controller IAM

The account operator owns controller/SSO policies. CDK does not provision or
mutate them. In addition to existing capture/recovery permissions, the required
policy shape is the following (replace example account/IDs/CIDR with reviewed
values derived from the deployed shared stack; this is documentation, not a
repository-managed IAM resource):

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "OpenReviewedAg4Tunnel",
      "Effect": "Allow",
      "Action": "ec2-instance-connect:OpenTunnel",
      "Resource": "arn:aws:ec2:eu-west-1:123456789012:instance-connect-endpoint/eice-0123456789abcdef0",
      "Condition": {
        "StringEquals": {
          "aws:RequestedRegion": "eu-west-1",
          "aws:ResourceTag/borrowser:purpose": "ag4-shared"
        },
        "NumericEquals": {"ec2-instance-connect:remotePort": 22},
        "NumericLessThanEquals": {"ec2-instance-connect:maxTunnelDuration": 300},
        "IpAddress": {"ec2-instance-connect:privateIpAddress": "10.77.0.0/26"}
      }
    },
    {
      "Sid": "ObserveAg4Transport",
      "Effect": "Allow",
      "Action": [
        "ec2:DescribeInstanceConnectEndpoints",
        "ec2:DescribeSecurityGroups",
        "ec2:DescribeNetworkInterfaces"
      ],
      "Resource": "*",
      "Condition": {"StringEquals": {"aws:RequestedRegion": "eu-west-1"}}
    },
    {
      "Sid": "ReadExactAg4SharedAuthority",
      "Effect": "Allow",
      "Action": ["cloudformation:DescribeStacks", "cloudformation:GetTemplate", "cloudformation:ListStackResources"],
      "Resource": "arn:aws:cloudformation:eu-west-1:123456789012:stack/borrowser-ag4-shared/01234567-89ab-cdef-0123-456789abcdef",
      "Condition": {"StringEquals": {"aws:RequestedRegion": "eu-west-1"}}
    }
  ]
}
```

The helper requests an integer duration no greater than its individual command
bound (and no greater than 300 seconds). IAM's 300-second ceiling does not replace
that check. Endpoint resource tags authorize the endpoint, **not the destination
instance**; Rust binds the requested private IPv4 to the verified run-owned
instance. Describe actions require `Resource: "*"`; exact-ID requests and regional
conditions remain mandatory. Existing `DescribeSubnets` is reused. No
`SendSSHPublicKey`, instance profile, guest credentials, controller IAM-management
permission, shared mutation permission, or EICE cleanup permission is needed.
See [AWS's OpenTunnel permission contract](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/permissions-for-ec2-instance-connect-endpoint.html).
Preflight validates the read behavior; the first authorized tunnel validates
OpenTunnel authorization. It does not simulate or introspect IAM policies.

### Shared update and separate authorization boundaries

The eight-resource shared stack is already deployed. Do not recreate it,
initialize KMS again, or substitute a new execution role. All following stages
remain implementation/deployment stages of AG4.

1. Offline, use the unchanged reviewed shared input and pinned CDK to synthesize:

   ```sh
   JSII_NODE=/absolute/pinned/node /absolute/pinned/venv/bin/python \
     crates/conformance/aws/shared_app.py \
     --input /absolute/private/shared.json --out /absolute/private/shared-cdk.out \
     > /absolute/private/shared-template.json
   ```

   Compare semantic JSON against the retained deployed template. Require exactly
   the three additions and appended execution-role statement above, unchanged
   original resource properties/outputs, and termination protection in the CDK
   assembly. Retain raw and canonical template digests. Fresh authenticated reads
   later must confirm the baseline is still current.
2. Only after separate authorization, obtain targeted baseline evidence for the
   exact shared StackId, template, physical IDs and termination protection, then
   create **one UPDATE change set**, with a durable unique request token:

   ```sh
   /absolute/pinned/aws --region eu-west-1 --no-cli-pager --no-cli-auto-prompt \
     cloudformation create-change-set --stack-name "$AG4_SHARED_STACK_ID" \
     --change-set-name "$AG4_CHANGE_SET_NAME" --change-set-type UPDATE \
     --client-token "$AG4_CHANGE_SET_TOKEN" --capabilities CAPABILITY_IAM \
     --template-body file:///absolute/private/shared-template.json
   ```

   Use externally supplied deployment credentials, separate from capture
   permissions. Do not switch the existing stack execution-role setting.
3. Inspect `DescribeChangeSet` for that exact returned ARN. Require three `Add`
   actions and `CaptureExecutionRole` `Modify` with `Replacement=false`; reject
   removals, replacements or unrelated changes. Resolve ambiguous submission by
   reading that exact change-set identity, never blindly submitting another.
4. Require separate authorization before `execute-change-set --change-set-name
   <exact-arn>`. Execute once. Observe terminal success/rollback and keep evidence.
   Verify protection remains enabled, all eight prior physical IDs unchanged,
   the new endpoint/SG identities/effective rules and the exact canonical digest.
   No shared deletion or protection disablement is part of this update.
5. Update private operator configuration with the exact StackId and approved new
   canonical digest, remove `controller_cidr`, and use fresh matching source,
   native controller and Linux artifact identities. The controller derives IDs;
   do not copy EICE DNS or an eleven-ID inventory into operator configuration.
   Have the account operator separately review the controller IAM prerequisite.
6. Only after local qualification and the reviewed shared update, separately
   authorize **at most one** fresh normal `canvas/cascade` operation. A failed
   run does not authorize another. Broader fault injection remains a later stage.

Initial shared setup and AWS-managed EBS initialization are historical completed
prerequisites. Shared cleanup is never delegated to a disposable operation.

SSO resolution stays outside AG4. An operator can export already authenticated
credentials into private memory and supply the three credential environment
variables. Never print/log the export, use `eval`, or persist credentials.
Credentials do not refresh within AG4; review their lifetime for the 30-minute
work allowance and separate 15-minute cleanup budget.

The same private AWS invocation policy applies to capture and cleanup-only.
Before invoking AG4, authenticate outside the runner and export
`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and, for temporary credentials,
`AWS_SESSION_TOKEN`. Missing, empty, control-containing or oversized values are
rejected before any AWS command. AG4 neither acquires nor refreshes credentials.
Expired credentials require external reauthentication followed by cleanup-only.

Every AWS child, including version checks, starts with a cleared environment.
One private empty `0700` temporary directory supplies an explicit `HOME` for the
invocation. The same owner keeps it alive through version checks and every normal
or cleanup-only AWS command, then removes it. Merely unsetting `HOME` is not
isolation on Unix: Python can discover the account home through the password
database, and Botocore independently searches `~/.aws/models`. The runner never
reads or modifies the operator's AWS home, and recovery needs no original bundle.
Only those credentials, that private HOME, fixed `/usr/bin:/bin` PATH/C locale and `/bin/sh` shell, disabled CLI retries,
metadata and pager settings, and an optional explicitly supplied `AWS_CA_BUNDLE`
are installed. Configuration, shared-credential and Boto files are set to
`/dev/null`; configured endpoints are explicitly ignored. Inherited endpoint
overrides (global or service-specific), profiles, role/web-identity/container
providers, credential helpers, SSO/login configuration, plugins/model paths,
proxy variables and unrelated CLI settings cannot enter this path.
Ordinary AWS API HTTPS uses bundled trust roots unless `AWS_CA_BUNDLE` names an absolute,
bounded (1 MiB) reviewed public certificate file without private keys. TLS
verification stays enabled. No credentials are serialized or logged; AWS error
diagnostics redact the supplied credential values. These settings follow the
[AWS CLI environment/configuration interfaces](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-envvars.html).

### Private EICE helper and ordinary SSH identity

The authenticated `DnsName` for the exact verified endpoint must have the shape
`eice-<id>.<opaque>.ec2-instance-connect-endpoint.eu-west-1.amazonaws.com`.
The first label must match the verified endpoint ID; the opaque label is not
interpreted or generated. The full hostname is bounded to 253 bytes and every
label to 63 lowercase DNS characters, with no empty labels or leading/trailing
hyphens. Only that exact regional service suffix is accepted. The ordinary
`ec2-instance-connect.eu-west-1.amazonaws.com` API suffix is not an EICE tunnel
hostname. The authenticated value is passed unchanged to `open-tunnel`; there
is no hostname construction, alternate suffix selection or discovery. This
matches the [AWS CLI EICE response example](https://docs.aws.amazon.com/cli/latest/reference/ec2/describe-instance-connect-endpoints.html).
The pinned CLI also exposes a separate FIPS DNS field; it is not required by
this reviewed ordinary IPv4 path and is not accepted as a compatibility guess.

The source-stamped controller dispatches `--ag4-eice-proxy <descriptor>` before
harness/comparison initialization. `TunnelDescriptor` is the concrete process
handoff, never recovery authority: bounded 16-KiB strict JSON, exclusively created
0600 without following links in the invocation's owned 0700 temporary directory,
alongside the SSH key and known_hosts. It contains no credentials, private key,
artifact bytes, arbitrary executable/argv or flags. It is removed with that
directory only after command quiescence. Unresolved ownership preserves the
private invocation directory and empty AWS HOME for local investigation, not as durable operation state. Resolved cancellation still removes them.

The descriptor carries `aws_cli_path`, device, inode, byte length and SHA-256
from the parent's qualified AWS CLI 2.37.13. The path must be canonical absolute,
UTF-8, control-free and free of symlinks in every component; its target must be
a regular executable owned by root/current UID without group/world write. The
parent hashes before and after version qualification. The helper reopens with
`O_NOFOLLOW`, hashes bounded contents and checks file/path identity again, then
uses absolute-path `exec` with the fixed argv below. There is no PATH lookup or
operator-config reload. This assumes a trusted local filesystem and trusted
same-UID/root actors through exec, including the packaged CLI's libraries;
pathname revalidation cannot provide an impossible atomic guarantee against
replacement after the final check. No executable abstraction is introduced.

```text
<verified absolute aws_cli_path>
  --region eu-west-1
  --no-cli-pager
  --no-cli-auto-prompt
  ec2-instance-connect open-tunnel
  --instance-connect-endpoint-id <verified ID>
  --instance-connect-endpoint-dns-name <authenticated DNS>
  --private-ip-address <verified primary private IPv4>
  --remote-port 22
  --max-tunnel-duration <bounded integer>
```

No instance/endpoint discovery, local listener, multi-connection mode, debug,
profile selection, prompt or TLS-disable switch exists. The helper requires
nonterminal streams, validates the parent's fixed environment and reconstructs
that allowlist for exec. Hostile `AWS_CLI_AUTO_PROMPT` is excluded; the command-line
disable is additionally unconditional. Stdin is tunnel protocol data, never
terminal prompt input.

OpenSSH targets `ag4@<instance-id>` with `HostKeyAlias=<instance-id>`. known_hosts
binds that exact instance ID to the console-authenticated guest ed25519 key.
The endpoint/DNS/IP never becomes SSH host identity. The operation key,
authorized_keys, remote commands and strict host verification remain unchanged;
there is no EC2 Instance Connect guest-side key injection. `ProxyCommand` contains
only the fixed controller path, helper flag and private descriptor path, quoted
for POSIX shell and with literal `%` doubled for OpenSSH expansion. The fixed
ProxyCommand precedes `ProxyJump=none`: OpenSSH 10.2p1 otherwise treats the latter
as the first proxy choice and ignores the helper. `-F /dev/null`, explicit key,
no agent, no multiplexing and `/bin/sh` isolation remain mandatory.

The process tree is `Owner → OpenSSH → ProxyCommand shell → helper → exec AWS
CLI → CRT/WebSocket workers`. All inherit the owned process group; the helper
creates no detached session or listener. OpenSSH sends SIGHUP to its proxy
without waiting. Successful root exit therefore starts observation, not command
completion. The command owner implements a candidate phase composition within
one five-second monotonic cleanup authority, starting once at root exit/failure:

| Boundary | Absolute deadline |
| --- | --- |
| Successful EICE natural-shutdown opportunity | T0 + 2 seconds |
| Latest planned KILL transition | T0 + 3 seconds |
| TERM grace | min(actual TERM dispatch + 1 second, T0 + 3 seconds) |
| Hard completion authority | T0 + 5 seconds |

The two-second forced/finalization allocation is explicit; it is not a residual
post-TERM tail. Earlier failure bypasses natural waiting. Late TERM gets shorter
grace, never a reset deadline. Poll waits are clamped to the next phase/command
boundary, with fresh monotonic observations after I/O and native group scans.
A phase-bounded scan reports executing members, complete no-executing-members
observation, or intermediate-deadline exhaustion. An incomplete scan never proves
quiescence. Malformed/conflicting observations and signaling errors remain failures.

All authoritative PGID observation and signaling finish while the exited root
remains unreaped and reserves its identity. Required pipe EOF and a complete
observation of no executing group members precede finalization. Only then is the
root reaped. No production PGID lookup or signal follows root reaping. Linux
finishes adopted-child reaping through its kernel subreaper relationship;
macOS verifies only its kernel-owned direct children. Both check the original
hard deadline before accepting completion. No production decision depends on
post-ownership bare PID/PGID disappearance. Authentication sanitization and
final diagnostic formatting follow ownership finalization.

Forced descendant cleanup makes apparent SSH success an execution failure;
unresolved observation/signaling/EOF/reaping poisons the owner. Later numeric
absence cannot repair an earlier ownership error. No parser/comparison starts
with unresolved ownership. CloudFormation cleanup needs no EICE tunnel and can
proceed after an ordinary transport failure once the owner is quiescent.

The 2/1/2 composition is a candidate, not a cross-platform qualification claim.
Fresh native macOS arm64 and Ubuntu 24.04 x86-64 evidence must demonstrate complete
production ownership finalization by T0 + 4 seconds, retaining one second of
headroom. This acceptance target is not a production timeout. It reserves half
the forced allocation against scheduling, observation and reaping variation;
passing just below five seconds is insufficient. The delayed 1.2-second natural
scenario fits the candidate natural window, but historical measurements do not
qualify the changed source. If the margin fails, retain the failure and review
it without expanding the five-second authority.

### Authentication-safe transport diagnostics

All EICE/OpenSSH/AWS-CLI stderr follows: **bounded capture → authentication
sanitization → bounded excerpt/truncation marker → formatting/persistence**.
The owner keeps draining late proxy stderr and discards after the 256-KiB capture
cap; overflow withholds the whole retained diagnostic because authentication
context may be incomplete. Actual access-key, secret-key and session-token
values are removed, including whitespace-wrapped values and a credential suffix cut by EOF. Percent encoding
is decoded for matching (at most twice); ambiguous remaining encoding is withheld.
Authorization/Proxy-Authorization, X-Amz fields, credential/signature/signed-header
context, wrapped field names and malformed request context are treated
conservatively. From the first authentication context through the remaining
message, content is withheld; preceding benign failure context is retained.
No assumption is made that ordinary pinned CLI failures cannot print such data.

Sanitization precedes `Failure`, controller stderr, upload excerpts, qualification
evidence and durable summaries. Both upload attempts retain separate bounded
nonsecret evidence. Existing 1,024-character durable failure fields and 16-KiB
record bound are unchanged. No descriptor dumps, keys, credentials, artifact
bytes, arbitrary stdout or AWS debug logs are retained. There is no FIFO,
additional diagnostic-stream framework or generic secret scanner.

### Corporate TLS qualification gate

The pinned CLI's [EICE WebSocket implementation](https://github.com/aws/aws-cli/blob/2.37.13/awscli/customizations/ec2instanceconnect/websocket.py)
constructs a default packaged-CRT `TlsContextOptions`; it does not forward the
ordinary Botocore `AWS_CA_BUNDLE` setting. It reads uppercase `HTTP_PROXY`, then
`HTTPS_PROXY`, with its own `NO_PROXY` handling. Production AG4 inherits none of
these proxy inputs and introduces no speculative replacement input. CRT trust
behavior must be qualified with this exact installed CLI on the supported Mac;
ordinary AWS HTTPS success proves neither WebSocket reachability nor trust.

Local loopback qualification with synthetic credentials reached a controlled
proxy and a TLS ClientHello with verification enabled. Both negative cases exposed
the packaged CLI's `handshake_response_headers` callback failure (`NoneType`),
which required the existing bounded command timeout/TERM path; cleanup succeeded.
This does not qualify the corporate path or a successful authenticated tunnel.
No production monkeypatch, `--no-verify-ssl`, global Mac trust change, arbitrary
proxy inheritance or corporate-control bypass is permitted. If the actual
approved path needs a supported explicit trust/proxy input, stop for review;
if it cannot safely support the pin, report a prerequisite blocker within AG4.

## Toolchain and immutable artifacts

`aws/toolchain.json` records Python 3.12.15, Node 22.23.3, AWS CLI 2.37.13,
OpenSSH 10.2p1 and Rust 1.92.0. `requirements.lock` pins every Python dependency
with hashes, including CDK 2.272.0 and constructs 10.8.1. Install the lock into
the specified Python virtual environment with hash enforcement. Do not use the
CDK library's bundled AWS CLI asset as the deployment CLI. The Linux build
container digest is recorded in `tools/conformance/ag4-builder.Dockerfile`.
That digest's installed `1.92.0-x86_64-unknown-linux-gnu` toolchain must report
Rust 1.92.0; preparation selects it explicitly without asking rustup to install
the repository's additional editor/lint components in the build container.

Supported controllers are native Ubuntu 24.04 x86-64 and native macOS arm64 on
the documented qualification baseline. Native process-owner qualification is
required on both. Other controller OS/architecture combinations are rejected.

Prepare both artifacts **before committing**, from the intended working tree:

```sh
sh tools/conformance/prepare_ag4_artifact.sh --out /absolute/private/ag4-bundle
```

Docker and public dependency download access are preparation prerequisites.
If the network uses an additional trusted issuer, explicitly pass
`--build-ca-file /absolute/outside-repository/public-ca.pem`. This accepts only
a bounded public certificate bundle, mounts it read-only for Cargo's container
downloads and preserves TLS verification. It is never part of the source tree
or delivered to the capture host. Docker's builder image/cache stays local.
Each build/identity-check container is explicitly created with a random
preparation-only name and ownership label, attached by exact ID, and removed
after matching name/label/ID inspection. The script confirms targeted absence;
terminating a Docker client or relying on `--rm` is not container cleanup.

The preparation-only process helper handles interruption, termination, command
failure, polling exceptions and the 30-minute deadline through bounded finally
cleanup. It keeps the root unreaped through process-group signaling, sends TERM
then KILL after one second where needed, observes native group termination and
reaps its children (including adopted Linux descendants). macOS Python builds
without `os.waitid` use the public native waitid ABI. Command cleanup has five
seconds; container cleanup gets a separate 30 seconds plus the current command's
five-second cleanup bound. Additional interrupts are recorded while cleanup
runs; they do not skip it. The foreground build tools must not escape their
process group. A failed observation/cleanup is reported, never successful prep.

An unresolved Docker operation retains `preparation-container.json` in the
private output directory with the exact name, label and ID if known. Inspect
only that named container, verify its recorded ownership, then stop/remove that
container and confirm its absence. Keep the record until confirmed; never scan
or remove unrelated containers. An ambiguous create followed by an empty lookup
does not erase this recovery identity. This is local artifact preparation, not
an alternate AWS resource lifecycle.

The script creates a temporary Git index and object store. It snapshots current
working-tree bytes, including staged/unstaged changes, new source, and deleted
files, without changing the real index or working tree or exporting old HEAD.
It rejects symlinks and unreviewed new input types, excludes operator secrets
and generated runtime artifacts, and verifies the exported file set and bytes.
The checked-in localhost TLS test fixtures remain ordinary public test inputs.
Do not edit source during snapshot creation; detected changes abort preparation.
Both builds consume the same read-only export and explicit Git tree stamp.
`--snapshot-only` prepares that export without building. `--aws-acceptance`
also compiles the explicit accepted-create-response fault hook.

The bundle contains the controller, Linux harness, `artifact.json`, source tree
identity and source archive. The AG4 entry point requires matching source,
fixture-byte hashes and Chromium pin identity, then verifies the Linux ELF and
binary SHA-256. The deployed harness is checked again after upload.
`build.rs` accepts an optional explicit stamp: ordinary AG1/AG2/AG3 builds and
commands do not require artifact preparation or any AWS configuration.

## Invocation and phase boundary

Copy `aws/operator.example.json` outside the repository and supply real reviewed
account/network/role/tool paths and dated regional prices. Point `cdk_app` at
the bundle's source export. The controller verifies that the CDK source and
browser pin match its own snapshot. Never put credentials in this file.

```sh
/absolute/private/ag4-bundle/borrowser-conformance-controller \
  --compare-aws --config /absolute/private/ag4-operator.json \
  --artifact-dir /absolute/private/ag4-bundle \
  --state-dir /absolute/private/ag4-operations canvas/cascade
```

Omitting the fixture selects the current two runnable fixtures in registry
order. Every invocation creates a new random 128-bit run ID, including repeated
executions of identical fixtures. Stack names are `borrowser-ag4-<run-id>`.

Bootstrap automatically installs the pinned Chromium archive/checksum and
Ubuntu snapshot dependencies, installs the exact AG2 AppArmor user-namespace
rule, creates the non-root capture user and operation public SSH key, and emits
a readiness marker and Ed25519 host key to the instance console. The controller
retrieves this key through authenticated AWS API access for the verified
instance. SSH uses strict host verification, an isolated known-hosts file,
explicit identity, no agent, proxy, user configuration or multiplexing.

Bootstrap timing is composed, not a set of independent timeouts assumed to fit.
The first real run (2026-10-14, `328e5bf58c5991bd07a403dfdf542b0e`) completed
package-index retrieval in about 166 seconds. Installation requested 52 packages;
downloads were still progressing at package 37 when cloud-init reported failure,
about 240 seconds after installation began. This matches the old install limit,
but the console did not expose its exit status or establish the cause of the
per-download delays. The old 180 + 240 + 180 seconds already filled the entire
600-second readiness window without accounting for other work; curl retries
could also exceed its per-attempt limit.

The reviewed sequence now bounds each entire phase with GNU `timeout`, including
all retries and commands in that phase. Each phase has a five-second TERM-to-KILL
allowance; failure stops the sequence rather than retrying bootstrap.

| Bootstrap phase | Seconds | Included work |
| --- | ---: | --- |
| `setup` | 30 | Guest stop timer, disable background apt, write snapshot sources |
| `apt_update` | 180 | Complete index update and its bounded retries |
| `apt_install` | 420 | Complete dependency installation and retries |
| `chromium_download` | 180 | All curl attempts, retry delays and connection time |
| `archive` | 45 | Digest check, extraction, permissions and archive removal |
| `sandbox` | 15 | AppArmor file, parser and loaded-profile verification |
| `ssh_prepare` | 10 | Capture user/directories and checked-in wrapper installation |
| `ssh_config` | 5 | Operation-specific authorized key, permissions and sshd drop-in |
| `ssh_validate` | 5 | Strict `/run/sshd` prerequisite, then unchanged `sshd -t` validation |
| `ssh_restart` | 10 | Unchanged `systemctl restart ssh` |
| `ssh_probe_loopback` | 5 | Loopback TCP 22 identification, four-second inner socket deadline, at most 255 bytes |
| `ssh_probe_private` | 10 | Bounded local primary-interface discovery and the same four-second TCP 22 identification check |
| `ready` | 5 | Read host public key and publish the existing readiness marker |
| Phase sum + conservative kill allowances | **985** | 920 + 13 × 5 |
| Controller readiness | **1,080** | Strictly exceeds 985 by **95 seconds** |

The 420-second installation allowance uses the observed roughly 240 / 37 seconds
per requested download as a sizing observation (52 downloads project to about
337 seconds), with time for remaining downloads and package processing. It is
not a diagnosis of network behavior or evidence of successful guest installation.
The 95-second margin covers boot/startup, shell overhead and console visibility /
polling; AWS or guest failures can still exhaust the deadline.

Readiness is admitted only with at least **1,620 seconds** remaining in the
30-minute work budget: 1,080 for readiness plus a 540-second reserve for the
existing upload (300), identity check (20), execution (120), retrieval (60), and
40 seconds of controller-command/validation overhead. Thus preflight/deployment
has at most three minutes to leave a full readiness-and-capture window. The
CloudFormation ten-minute creation bound is unchanged, but a slower deployment
fails this admission and reaches cleanup; readiness is never silently shortened
to a budget that cannot fit bootstrap. The independent cleanup budget remains
15 minutes; the guest's 45-minute stop safeguard remains supplementary. These
are bounded attempts, not guarantees against uninterruptible kernel work or
billing ceilings.

On failure bootstrap emits one small console line:
`BORROWSER_AG4_BOOTSTRAP_FAILED <run-id> <phase> <exit-status>`.
Only the twelve phase names above and a nonzero numeric shell status are valid.
The actual status is preserved: for example apt failure 100, curl failure 22,
timeout 124, or forced timeout termination 137. No command text or package output
is included in the marker. The controller checks the exact run/instance identity,
rejects malformed or conflicting markers, and immediately reports a matching
failure as the primary error. Failure never authorizes SSH or capture; only the
unchanged `BORROWSER_AG4_READY` marker remains success authority.

The second real run (`a9a515b98c5684638b5b5e418d4640fd`, source
`dc4fd3029a3c80a38f5e4b0977d654eebe521f2f`) reached the former combined `ssh`
phase and reported status 255; ordinary cleanup was confirmed. Retained console
reads were empty, so the failing command and underlying cause are not established.
The SSH split preserves its aggregate 30-second allowance and all commands/policy;
it identifies preparation, configuration, validation or restart failures without
creating a general tracing protocol. Every failure stops later phases, including
readiness. In particular, `sshd -t` remains mandatory: Noble OpenSSH checks both
configuration validity and host-key sanity. A validation failure cannot be
bypassed to publish readiness.

A nonbillable Ubuntu 24.04 x86-64 container check using the reviewed package
snapshot reproduced `ssh_validate / 255` with OpenSSH
`1:9.6p1-3ubuntu13.19`: `/run/sshd` was absent. The third real run
(`4c4da4c95a9d6e17ff97ee446b5a3888`, source
`e1579dafe9ea68c6ad77e38ae90b8e4777dbc043`) then retained the same explicit
OpenSSH diagnostic immediately before its run-bound `ssh_validate 255` marker.
No capture ran; normal cleanup was confirmed. This establishes the standalone
validation prerequisite for that guest without guessing why its earlier boot
had not materialized the directory or attributing the second run to that cause.

AG4 deliberately validates sshd before asking systemd to restart it. Ubuntu's
packaged `ssh.service` declares `RuntimeDirectory=sshd` and mode 0755, but that
service transition has not yet occurred when AG4 invokes `sshd -t` directly.
Within the existing five-second `ssh_validate` phase, AG4 creates `/run/sshd`
only when absent, with explicit root owner/group and mode 0755. It then requires
a real, nonsymlink directory, numeric uid/gid 0, exactly mode 0755 and no entries
(including hidden entries). A dangling symlink is an existing conflicting path.
An existing wrong type, symlink, ownership, mode or populated directory fails
the phase; AG4 never repairs, replaces or empties it. The checks and unchanged
`sshd -t` must all succeed before `systemctl restart ssh`. This is ephemeral
runtime state for standalone validation; systemd remains the service lifecycle
owner afterward. No SSH policy, host key, authentication, phase bound or READY
marker format changed in that prerequisite correction.

`test_bootstrap.py` covers phase ordering/statuses, key modes, exact policy and
wrapper bytes with local helpers. Its optional
`BORROWSER_AG4_NOBLE_SSH=1` case must run only in a fresh, disposable root Noble
Docker container with `openssh-server`, `python3`, `systemd`, `iproute2`, `gzip` and generated host
keys installed, without a pre-existing `ag4` account or `/run/sshd`. Run
`python3 -m unittest discover -s crates/conformance/aws/tests -p test_bootstrap.py -v`
with the source mounted read-only and a disposable local bridge interface/default
route. It makes no external requests. The case exercises
the exact synthesized SSH sequence starting with an absent `/run/sshd`, proving
production creates its required root-owned 0755 empty directory. It checks real
file ownership, `sshd -t`/`-T`, unchanged rejection of invalid configuration and
missing host keys, and refusal to repair conflicting runtime state. Failed
validation never reaches the controlled restart or READY. A real restart in
this container fails because systemd is not PID 1; a test-only restart helper
checks successful ordering and readiness publication. That positive helper case
is not qualification of a real systemd restart. Both probes also run against a
real packaged sshd on actual loopback/private addresses; `ss` associates its
IPv4 port-22 listener with the owned daemon PID. Controlled private endpoints
exercise refusal, silence and invalid banners without sending client data.
Emulated container results are
neither native Ubuntu nor AWS evidence. The subsequent real run described below
qualified the runtime prerequisite and real restart, but not capture.

Run `4849d3b3fc2fd3df7adb3ec6c7535965` (source
`6ce29612d98162d9e66787aafa3c6d54e9e29266`) passed validation and real systemd
restart and published READY. The console key matched the controller's bound
known_hosts entry. The first SSH upload then timed out during banner exchange;
no artifact verification, Chromium execution or comparison occurred. Separately,
the command owner reported failure to terminate its group and blocked subsequent
commands, including in-process stack cleanup. Cleanup-only recovery subsequently
confirmed deletion and absence of the instance/root volume. Both failures remain
valid observations; neither underlying cause is established by later process
absence or successful recovery.

READY requires `ssh_probe_loopback` and `ssh_probe_private`, in that order,
after unchanged validation and restart. Each uses the existing guest Python
standard library: IPv4 TCP port 22, at most 255 bytes under a four-second
monotonic socket deadline, a printable CRLF-terminated `SSH-2.0-` identification
with nonempty software version. Neither sends client identification or
authentication data, nor logs the received banner. Refusal, silence, premature
EOF, malformed or oversized identification fails its separately named phase;
READY remains withheld. The loopback phase has five seconds. The private phase
has ten seconds including discovery and its four-second socket budget.

The private phase uses Ubuntu's existing `ip -j -4 route show default` and
`ip -j -4 address show scope global` to read the local kernel's interface state.
Each observation is capped at 8 KiB (one extra byte detects overflow). It
requires exactly one unicast default route without multipath, a valid non-loopback
device name, exactly one matching UP interface, and exactly one global primary
IPv4 address on it with a valid prefix length. The address must be literal
RFC1918 IPv4; missing, duplicated, secondary, malformed or overflowing state
fails closed. No DNS, metadata-service request, external connection or AWS
credentials participate. The public IPv4 is mapped to this private interface by
AWS and is not an address configured on the guest.

The two banner probes are the listener-readiness authority. They establish local
identification responses on both addresses, not cryptographic daemon identity or
external-path reachability. Ubuntu socket activation remains unchanged. EICE
changes only the controller-to-private-interface TCP path and SG topology;
the readiness sequence adds no probe. The earlier direct-public controller `/32`
could not be qualified behind Zscaler. An HTTPS egress observation did not prove
the SSH source; the new transport removes that source-IP contract entirely.

The real operation `862a8c18ca76026917f960088d0c721a` completed loopback readiness,
then both permitted upload attempts reported `Connection timed out during banner
exchange`. This establishes an external SSH identification timeout, not merely
an unproven TCP-connect failure. It does not establish contact with the expected
guest sshd, host-key exchange, authentication or the network cause. Both command
cleanups and normal stack cleanup succeeded; no Chromium execution occurred.

To keep the complete user data below EC2's 16-KiB limit, CDK embeds the existing
guest wrapper as deterministic gzip (no filename or timestamp, fixed OS header)
and base64. Explicit `gzip` and `iproute2` dependencies are installed in the
existing pinned-snapshot `apt_install` phase; gzip decompresses the wrapper during
`ssh_prepare` and iproute2 supplies the private-interface probe's `ip`. Retained
snapshot indexes contain amd64 `gzip` 1.12-1ubuntu3.2 and `iproute2`
6.1.0-1ubuntu6.4. There is no additional install phase or enlarged deadline. Tests
check the installed wrapper is byte-identical to the checked-in source; this
changes packaging only, not guest execution or the capture format.

The command owner captures OS errors immediately after failed group signaling
and retains the signal name, OS text and raw errno. TERM and KILL diagnostics
survive later cleanup errors. Only ESRCH retains its existing meaning that the
group is already absent; other errors still poison the owner and prevent later
commands/comparison. Diagnostics contain no credentials or child output. A
separate explicit local qualification exercises the pinned OpenSSH client against
a loopback listener that accepts but sends no identification, with a 1-MiB stdin
payload, the production ten-second connect limit, and actual owner cleanup:

```sh
BORROWSER_AG4_COMMAND_SCENARIO=ssh-banner \
BORROWSER_AG4_TEST_SSH=/absolute/path/to/pinned/ssh \
cargo test -p borrowser-conformance --locked --bin borrowser-conformance \
  aws::command::tests::isolated_owner \
  -- --exact --ignored --nocapture --test-threads=1
```

The listener is bound before launch and waits for client EOF; no external network,
AWS credentials, authentication or guessed race sleeps are involved. Test-only
diagnostics record root exit status, input failure, TERM/KILL outcomes and final
quiescence. Helper/loopback results do not qualify the real controller network.

The native macOS pinned-client reproduction returned SSH exit 255 and input
EPIPE. Its complete native group observation already reported no running
members, but the owner still issued SIGTERM; that syscall returned EPERM (1).
The owner now avoids signaling when its current complete observation establishes
no running group members, retaining the leader identity until exit, pipe and
reaping checks complete within the same bound. Unknown/failed observations still
attempt termination and remain unresolved; a failed signal is never reclassified
as success by a later observation. The original live failure did not retain errno,
so this is a demonstrated local mechanism, not proof of that run's underlying
OS error or the cause of its external banner timeout.
If a group was observed running but exits before signaling, a non-ESRCH signal
error still remains unresolved. A natural-exit race regression preserves this
conservative outcome and verifies that subsequent commands are refused. The
separate closed-input helper synchronizes input acceptance, closes stdin and
stops itself until the owner terminates it, testing TERM/KILL cleanup without
guessing an exit race.

The controller uploads the verified Linux harness through SSH stdin, checks its
digest and artifact identity, and executes its existing `--chromium` command
once through the checked-in `aws/guest_capture.py`. Bootstrap installs that exact
wrapper read-only with the Ubuntu-snapshot Python package; its bytes are part of
the synthesized/template identity and the controller's source verification.
Fixtures remain embedded bytes from the same source snapshot. There is
no SSH fallback and no execution retry after an ambiguous outcome.

Artifact upload retains the existing maximum of two attempts within one
300-second upload deadline, capped by remaining capture-work time. Both attempts
are upload-only, before artifact verification or any Chromium execution. Each
failed attempt records its number and whether the command returned `Output` or
`Failure`. `Output` retains the SSH exit code and at most 2,048 UTF-8 bytes of
stderr. `Failure` retains at most 8,192 UTF-8 bytes of primary failure (including
the command owner's bounded SSH stderr prefix when supplied) and 2,048 bytes of
cleanup failure. The command-owner `Failure` type does not expose an exit code;
that case explicitly reports `ssh_exit=unavailable`. Truncation preserves UTF-8
and includes a truncation marker within each limit. Supplied AWS credentials are
redacted before truncation. Diagnostics never format command input, artifact
bytes or stdout, and never read SSH private-key contents.

Successful command cleanup does not erase the transport failure. If the second
upload succeeds, the first failure remains on the invocation's diagnostic
stream, outside the comparison output. If upload fails, its returned error
contains every attempted outcome, bounded below 24 KiB before the enclosing
invocation diagnostic formatting. Unresolved command cleanup still aborts
immediately, preserving earlier attempt evidence and issuing no further upload.
The durable operation record keeps its existing 1,024-character primary-failure
summary; the invocation diagnostics carry the fuller bounded attempt details.

Real operation `3c8e94eb303ce16753a02e6edbae8c1c` established successful guest
SSH validation, systemd restart, loopback banner readiness and READY publication,
followed by artifact-transfer failure and successful ordinary in-process cleanup.
The earlier upload implementation discarded both ordinary failure outcomes, so
that operation's evidence does **not** establish a banner timeout or a network
root cause. This diagnostic correction cannot reconstruct the lost details.

The wrapper drains stdout and stderr separately, limiting only the report to
64 KiB and guest diagnostics to 256 KiB. It imposes no file-size rlimit on the
harness or Chromium; browser profiles/state remain governed by the existing
AG2 restrictions and the host's storage limit. Either stream overflowing,
nonzero execution, cancellation or the 110-second guest deadline fails capture.
The report is published by rename only after complete streams and successful
harness exit; failures cannot publish `capture.json`. Diagnostic overflow keeps
a bounded prefix and fails execution. Partial JSON is never accepted: subsequent
ingestion validates the complete published result.

SIGINT/SIGTERM only record cancellation; handlers never throw through child
acquisition or cleanup. Safe points after `Popen` ownership, during stream/exit
polling and around publication stop capture. A signal received during failure
cleanup preserves the primary failure and cannot bypass TERM's one-second grace,
KILL or the four-second child wait. Further handled signals are recorded without
abandoning these bounds. Primary failure, interruption and cleanup failure are
reported separately. A cancellation observed during rename removes this
invocation's unpublished-as-success result; it cannot authorize a successful
capture. The wrapper retains the root until all required group signaling finishes.
This guest cleanup remains supplementary to CloudFormation disposal.

On execution failure the controller preserves the SSH exit/failure and up to
2 KiB of SSH stderr. If commands are quiescent, capture is not cancelled and work
time remains, it makes one best-effort read of the first 8 KiB of
`capture.stderr`, for at most ten seconds with five seconds reserved for command
cleanup. This read consumes only capture-work time. Empty/unavailable diagnostics
are explicit, never replace the primary failure, never retry capture and never
borrow the separate CloudFormation cleanup budget. There are no cloud logs or
capture archives. The operation record retains a bounded failure summary;
the invocation's diagnostic stream carries the retrieved prefix.

Retrieval is bounded before buffering beyond 64 KiB. Typed wire decoding rejects
duplicate/unknown fields, trailing/malformed JSON, missing observations, wrong
order/identity/source, incompatible browser identity/platform and unexpected
capture environment. Registry-owned `TestId` values and existing capture types
are preserved. Operational identity checks stay outside comparison output.

Only after capture validation, confirmed stack cleanup, and release of all
controller-command/runtime/signal ownership does `compare_captured` start the
Borrowser phase. Its AG3 projection, authored expectations and serialization
remain shared with local comparison. Failure emits diagnostics and no comparison
report.

## Durable operation lifecycle and recovery

`Record` remains version 1 with no EICE fields. `TunnelDescriptor`, current
operator/shared configuration, artifacts and OpenTunnel access are unnecessary
for cleanup-only. Historical deletion uses its original digest/binding/resource
and deletion-policy validation; new live capture additionally requires the exact
EICE topology. There is no weakened union validator or template-version system.

Two byte-for-byte historical fixtures were frozen before replacing synthesis:

| Fixture in `aws/tests` | Retained provenance | Raw SHA-256 | Canonical AG4 digest |
| --- | --- | --- | --- |
| `legacy-direct-ssh-template.json` | Qualified source `86db597ba53b6023ba6a5f37eb9a250d2f13bc42`, original synthesized fixture | `ef990db679e457a04d79b9d249e3401838c1df7144dfaab61e1e7c7c304d7875` | `6912340031ac8362d8e3e0cd339db0f2588cc60d20008be0b3efb56f8ec1b164` |
| `legacy-direct-ssh-deployed-template.json` | Operation `862a8c18ca76026917f960088d0c721a`, source `e8574e7bb47442dfb9417597fb901776f5a30ef7`; retained submitted template, GetTemplate and operation record agree | `76aeb2945cfd22aab4d49824ea79f62933816f6b4d1ba6d8b065fe2f322729bd` | `bd467104e870245cddfeac1047a5cc0aa5afec10d20b5c94ba9fa36a281f9a9b` |

Tests assert these hashes, exercise current deletion authorization and reject
mutated digest, ownership, resources and deletion semantics. Neither fixture
was reconstructed or normalized. Historical templates are not live defaults.

Each outside-repository mode-0700 operation directory holds a mode-0600
`operation.json` and a stable advisory lock. Updates use temporary-file sync,
atomic rename and directory sync (plus full file sync on macOS). Only one
controller/recovery process may hold the lock. Copying state to concurrent
machines or externally mutating infrastructure during an operation is unsupported.
New state/operation directories also sync their parent before submission.

The record contains version, run ID, account, region, deterministic stack name,
verified stack ARN once available, template digest, creation/capture/cleanup
states, bounded primary/cleanup failures, and instance/root-volume IDs only
when observed for execution or targeted cleanup. It contains no credentials,
SSH private key, artifact bundle, fixture content, full API log or evidence log.

Creation transitions from `Prepared` to durably persisted `Submitted` **before**
`CreateStack`. A persistence failure prevents submission. The creation request
token derives from the run ID; submission happens once, with CLI retries
disabled. Successful identity binding produces `Bound`. Lost responses leave
`Submitted`; recovery uses bounded exact-name lookup, verifies ownership, saves
the unique ARN and subsequently addresses only that ARN. Absence after an
ambiguous request remains unresolved. Recovery never launches or recreates.

The verified stack ARN must be durable before submitting deletion: deleted
stacks cannot subsequently be recovered by name. If storage keeps failing,
cleanup remains explicitly pending with a name-recoverable live stack. This
durability requirement is separate from ownership authorization. After fixing
local storage, repeat cleanup-only. Never discard the operation record.

The template digest hashes `borrowser-ag4-template-v1\n` plus compact UTF-8 JSON
with recursively sorted object keys, preserved arrays/values and no field
elision. Duplicate keys and noninteger number forms are rejected. Recovery
requests `GetTemplate --template-stage Original`; object and JSON-string body
representations use the same decoder. All infrastructure configuration is
embedded. Hashed `Metadata.BorrowserAg4Operation` also binds external creation
settings, which recovery compares against `DescribeStacks`. No separate supplied
CloudFormation parameters, transforms or dynamic image lookups are used.

`EnableTerminationProtection` must be present and match the binding for every
state except verified `DELETE_COMPLETE`. For that terminal state only, a binding
requiring `false` accepts either explicit `false` or omission: CloudFormation
cannot successfully delete a protected stack. Explicit `true`, null, malformed
values and omission in other states remain conflicts. No other ownership or
binding check is relaxed. The first live deletion exposed AWS omitting this field;
its original capture failure remains recorded when cleanup-only later confirms.

Cleanup-only recovery needs the record, pinned CLI and credentials, not the
original source, CDK installation, artifact, fixture selection or SSH key:

```sh
borrowser-conformance --aws-cleanup \
  --operation /absolute/private/ag4-operations/RUN_ID \
  --aws-cli /absolute/path/to/pinned/aws
```

Ownership verification authorizes deletion; it does not confirm completion.
The exact verified ARN must reach `DELETE_COMPLETE` and applicable observations
must establish disposal:

| Available identity | Additional confirmation |
| --- | --- |
| Instance and root-volume IDs | Complete matching stack records, terminated/absent exact instance, empty exact-volume query. |
| Instance ID only | Recover it from stack records and the root mapping where available; otherwise use the next case. |
| Root ID lost during rollback | Terminal CloudFormation disposal, verified delete-on-termination and creation-time tags, terminated/absent instance, complete empty run-and-stack-tag volume query. |
| No materialized instance established | Complete creation history tied to the create token and terminal CloudFormation handling; scoped instance/volume observations must establish no remaining resources. Missing physical IDs alone are insufficient. |
| Conflict, missing authority, failed observation | Remain pending/unresolved and preserve recovery data. |

API pagination and display paging are distinct. The CLI uses `--no-cli-pager`
and `--no-paginate`; a continuation token causes refusal. Empty collections must
come from successful, structurally complete responses with the exact filters.
Generic not-found errors, permissions failures, truncation, omitted collections,
unexplained absence and incomplete histories cannot confirm cleanup. No force
deletion, retained-resource escape, direct resource deletion or broad discovery
is provided. Primary and cleanup failures remain separately reportable.

After `DELETE_COMPLETE`, a complete observation of a matching instance in a
valid nonterminal state or a still-visible matching root volume is **pending**.
The same necessary targeted checks continue at five-second intervals within
the existing 15-minute/180-poll cleanup budget. All nonempty volume observations
must have the expected ID (when known), matching ownership tags and a recognized
state. Conflicting, ambiguous, malformed or incomplete observations fail
immediately; they are not pending observations to retry. No additional deletion
or creation is submitted while terminal cleanup confirmation settles. Persistent
presence exhausts the budget and leaves cleanup unresolved and recoverable.

## Bounds, cancellation and spending

| Stage/resource | Bound |
| --- | --- |
| Preparation / synthesis | 30 minutes before AWS / 60 seconds |
| AWS command | 20 seconds, plus at most 5 seconds for process cleanup |
| Read retries | At most 3, delays 1 and 2 seconds; CLI retries disabled |
| Creation | One submission, CloudFormation creation timeout 10 minutes |
| Deployment / host readiness | 10 minutes / 18 minutes; 5-second polling, at most 120 / 216 polls; full readiness + capture reserve required |
| Lost-response lookup | 5 minutes, at most 60 polls |
| Upload / execution / retrieval | 5 minutes (2 attempts) / 120 seconds (once) / 60 seconds (2 attempts) |
| Overall capture work / cleanup | 30 minutes / separate 15 minutes; at most 3 deletion submissions, 180 polls |
| Input/output | Harness 64 MiB; artifact metadata 16 KiB; capture 64 KiB; API and diagnostic streams 256 KiB each |
| Operation resources | One c6i.large, one 16-GiB root disk, one ENI/public IPv4, one SG, at most two fixtures |

The command owner drains nonblocking stdout/stderr fairly with bounded buffers.
Overflow, deadlines and cancellation close input, terminate the owned process
group, escalate after one second, and bound observation/reaping. Root exit and
pipe EOF are not sufficient; helpers holding pipes or remaining alive are
included. A failed group observation or cleanup prevents subsequent commands
and parser workers. This contract covers the foreground pinned tools, not
arbitrary commands that daemonize or escape their process group.

First interruption stops capture work but permits the independent CloudFormation
cleanup budget. A second interruption stops that attempt and preserves pending
recovery information. Uncatchable controller termination is handled by later
cleanup-only recovery. Temporary keys/source artifacts do not authorize deletion.

Cost admission is an estimate capped at USD 0.50: twice one hour of instance
and IPv4 cost, 16 GiB of gp3 for one hour (730-hour monthly conversion), 0.25 GiB
egress and a reviewed request allowance. All rates must be positive regional
micro-USD inputs reviewed within 30 days. The guest's 45-minute stop timer is a
supplementary compute safeguard. A stopped guest still retains billable storage;
guest, controller, network, credentials or AWS failures can leave resources
billable indefinitely. Neither deadlines nor admission estimates are billing
ceilings. Only confirmed disposal ends the operation's cleanup responsibility.

## Tests and opt-in AWS acceptance

Local EICE coverage adds strict shared/config/ENI identity, exact SG sets and
effective rules, eleven/two resource boundaries, external controller policy
documentation, the narrow role delta, fixed executable/argv handoff (including
`--no-cli-auto-prompt`), hostile PATH/config/prompt inputs, descriptor permissions
and lifetime, percent/shell quoting, natural/delayed/surviving proxy cleanup,
late stderr and authentication redaction across stream/excerpt/retention bounds.
The real historical fixtures exercise unchanged recovery authority.

Explicit native commands (no AWS calls):

```sh
BORROWSER_AG4_COMMAND_SCENARIO=proxy-timing \
cargo test -p borrowser-conformance --locked --bin borrowser-conformance \
  aws::command::tests::isolated_owner -- --exact --ignored --nocapture --test-threads=1

BORROWSER_AG4_TEST_SSH=/absolute/pinned/ssh \
BORROWSER_AG4_TEST_SSHD=/absolute/local/sshd \
BORROWSER_AG4_TEST_CONTROLLER=/absolute/source-stamped/controller \
cargo test -p borrowser-conformance --locked --bin borrowser-conformance \
  aws::ssh::tests::native_eice_stream_proxy -- --exact --ignored --nocapture --test-threads=1

BORROWSER_AG4_COMMAND_SCENARIO=eice-cli-local \
BORROWSER_AG4_TEST_AWS_CLI=/absolute/pinned/aws \
cargo test -p borrowser-conformance --locked --bin borrowser-conformance \
  aws::command::tests::isolated_owner -- --exact --ignored --nocapture --test-threads=1
```

The timing command takes eight samples each of natural, 1.2-second delayed,
TERM-responsive and TERM-ignoring proxy descendants using the actual owner.
Test-only authoritative milestones distinguish cleanup start, root-exit observation,
natural phase end, TERM/KILL, each pipe EOF, last pre-reap group-quiescence
confirmation, signaling complete, root reaping, native kernel-owned reaping and
complete owner quiescence. Both record families must contain 32 unique samples.
The acceptance test requires all complete production finalizations by T0 + 4 seconds.
The TERM deadline is a maximum, not a requirement to wait a whole second after a
late dispatch. Record actual dispatch lateness separately from that planned bound.

Supplementary numeric group/root/descendant disappearance measurements are labeled
`qualification_only_non_authoritative`. They occur after ownership and never
establish production correctness. A present numeric identifier can be reused;
an ambiguous or bounded-out observation is unavailable, not proof of an owned
survivor. Linux kernel-owned adopted-child reaping and macOS direct-child checks
remain separate authoritative events. No PID inventory/lifetime abstraction is used.

Historical failure: qualification commit `2a708c74c6f33a57abfac0451cab9b1a3373181b`,
tree `8f7029b68d9c7da8f7f73c4aa804000c88c758da`, GitHub Actions run
`38957426608`, attempt 1, failed during KILL sample 2. From cleanup start, TERM
was 3.979651 s, KILL 4.997921 s, stdout/stderr EOF 4.998172/4.998173 s, and root
reaping 5.020282 s. Group/native reaping was unresolved. Twelve OWNER and eleven
complete records were retained; this was not a successful 32+32 qualification.
The original artifact SHA-256 is
`c0b9807797ffae2346d4bbb4d513ac817f3045f155c6faa3663647d059ce1b81`.
The durable record is under
`~/.local/share/borrowser/reviews/ag4/native-proxy-timing-8f7029b68d9c7da8f7f73c4aa804000c88c758da/`.
Do not rerun that source. It demonstrates invalid internal composition, not that
five seconds is inherently impossible.

The earlier macOS raw timing and Chromium logs were lost from temporary storage.
Their previously reported timing maxima (natural 0.172400 s, delayed 1.209466 s,
TERM 4.052818 s, KILL 4.996157 s for supplementary absence) are historical summaries
only, not reverified evidence. Likewise retain the two historical `BrowserExited`
observations and reported pre-EICE reproduction without attributing them to EICE.
Fresh Chromium qualification is separate AG4 work after timing correction.

For each changed frozen source, run native macOS qualification first and preserve
its exact source/tool/platform identity and raw/parsed evidence. A new native
Linux publication/run requires separate authorization and a new tree-bound ref.
Neither historical summary nor one platform's passing result establishes the
other platform's qualification. Current results belong to the durable per-tree
review record; the new Linux run and broader live acceptance remain pending.

The existing `CI` workflow has one explicit `ag4_proxy_timing` job for pushes matching
`qualification/ag4-native-proxy-timing-*`. All other jobs in that workflow skip only
the `refs/heads/qualification/ag4-native-proxy-timing-` prefix; ordinary master/PR
behavior is unchanged. Before any build or timing execution, the job requires
checkout HEAD to equal `GITHUB_SHA`, a clean checkout, and `GITHUB_REF_NAME` to
equal `qualification/ag4-native-proxy-timing-` followed by the exact full lowercase
tree ID returned by `git rev-parse 'HEAD^{tree}'`. Missing, shortened, uppercase,
nonhex, extended or different-tree suffixes fail closed. The expected tree is not
embedded in the tracked workflow: the publication ref supplies the binding without
a self-reference. The job requires a
GitHub-hosted Ubuntu 24.04 Linux x86-64 VM, rejects containers and target-runner
overrides, records the exact checkout commit/tree and Rust 1.92.0 host identity,
and runs only the timing command above after its native test build. It uses no
AWS credentials, AWS calls or Chromium capture. The Linux owner uses
its actual subreaper/adopted-descendant path. Raw logs are uploaded even on
failure; success additionally requires both sets of 32 unique timing records,
complete authoritative milestone ordering and the T0 + 4-second margin.
The artifact `ag4-proxy-timing-linux-x86-64-<commit>-<attempt>` retains environment,
source tree, toolchain, build, raw timing and parsed complete records for 14 days.

Immediately after the separately authorized run, download and preserve the evidence
under `~/.local/share/borrowser/reviews/ag4/<qualification-id>/`. Preserve the original
downloaded artifact archive and its SHA-256, plus `environment.log`, `source-tree.txt`,
`toolchain.log`, `build.log`, `timing.log` and `records.json`; record the workflow run
ID, attempt, commit SHA and exact tree SHA alongside them. Verify these identities
against the authorized tree/ref and the run before accepting the evidence. If a
failed run lacks later files, preserve all available evidence and record the missing
files explicitly; do not manufacture complete records or accept partial qualification.
The bounded GitHub artifact is transport, not long-term AG4 authority. Do not defer
preservation until its retention period expires.

Local qualification must write evidence durably from the start, under the same
review-store root. Retain environment/platform identity, Rust 1.92.0 native host,
source/commit/tree identity, binary SHA-256, exact command lines and exit statuses,
build/raw timing logs, parsed summaries and a file-hash manifest. Reject translation
and target-runner overrides. Temporary execution directories and CI artifacts are
transport only. If losing a file would retract a qualification claim, it must not
exist only in `/tmp`, `/private/tmp`, `$RUNNER_TEMP` or temporary GitHub storage.

The new-source Linux qualification is pending; the old failed run remains evidence. Publication must
use the exact reviewed source snapshot: compare the published commit's Git tree
with the frozen snapshot tree and derive the qualification branch from that tree
before the single authorized push, then compare
the retained checkout/tree evidence with it. Do not substitute an older committed
source, open a PR to trigger unrelated capture jobs, or rerun a failure without
review. Do not publish or rerun a changed source without separate authorization.

The SSH command uses pinned OpenSSH 10.2p1 and the production helper with a test-only
argv-checking local stream server instead of AWS, the current local account
instead of `ag4`, generated keys, strict instance-ID known_hosts, real upload,
wrong-host-key rejection, tunnel failure and banner timeout. It creates no local
account/listener. The CLI command uses the actual CLI 2.37.13 and packaged CRT,
synthetic credentials and loopback proxy/TLS failures only. Its test-only proxy
input never becomes production configuration. These seams do not qualify a
successful corporate tunnel; preserve that distinction in evidence.

After separately authorized shared deployment and verification, the first live
EICE stage permits one normal `canvas/cascade` operation, **not** the multi-run
acceptance procedure below. Its evidence must establish the exact shared
identity; authenticated tunnel/private SSH identification; matching console-bound
host key and operation-key authentication; bounded upload with byte size and
duration; remote digest/source verification; exactly one AG2 execution; retrieval
and ingestion; confirmed stack deletion and absent exact run resources;
quiescent OpenSSH/shell/helper/CLI ownership; successful unchanged AG3 comparison;
and unchanged shared StackId/template/resource/EICE/SG identities afterward.
Successful SSH authentication/upload implies identification and host-key exchange;
do not enable verbose/debug logs or retain raw authenticated request context to
collect redundant evidence. A failure authorizes cleanup, not a second capture.

Ordinary tests are credential-free and nonbillable:

```sh
cargo test -p borrowser-conformance --locked
python3 -m unittest discover -s tools/conformance/tests -v
# In the pinned, hash-locked CDK virtual environment:
python -m unittest discover -s crates/conformance/aws/tests -v
cargo clippy -p borrowser-conformance --all-targets --locked -- -D warnings
```

The lifecycle uses scripted commands and a fake clock; the real command owner
uses isolated local helper processes. `aws/tests/synthesized.json` is a test
response produced by the real Python app with explicitly synthetic identities,
not an alternative deployment definition. Live tests are separately gated by
the `aws-acceptance` Cargo feature, ignored-test selection and explicit operator
environment variables. CI must never set those variables or enable that lane.

Preflight regressions use a recorded image-field fixture, exercise all root/
virtual mapping orderings, reject malformed/ambiguous/additional storage and
retain the image-identity and fixed-instance checks. Separate launched-instance
regressions reject additional mappings and a retained root. These are scripted
tests, not live guest evidence; the capture template's encryption/deletion
assertions and cleanup lifecycle tests remain in force.

`test_shared_stack.py` separately checks the eleven-resource shared boundary,
explicit routing, role trust/resource restrictions, deletion settings and
configuration rejection. Its actual-app subprocess uses a copied app and an
explicit synthetic `runner.json` fixture with no credentials; it also verifies
that unset pins refuse synthesis. Ordinary CI's existing Python discovery
includes these tests. They do not qualify deployment permissions or real routing.

The ordinary command-owner tests exercise normal calls and the actual cleanup-only
entry point, checking the live private home's contents and lifetime in each child.
An offline Python filesystem probe substitutes a temporary password-database home
inside the test process, reproduces fallback lookup and proves explicit HOME
blocks it. No test changes the operator's `~/.aws` directory.
Optional isolated-owner scenarios provide separate evidence:
`BORROWSER_AG4_COMMAND_SCENARIO=aws-model-library` with
`BORROWSER_AG4_TEST_AWS_PYTHON` uses the installed CLI's vendored Botocore library;
`aws-model-cli` with `BORROWSER_AG4_TEST_AWS_CLI` requires the exact pinned CLI
version and checks offline `--generate-cli-skeleton` model resolution. Both use
synthetic credentials and a synthetic service model under a temporary home.
Library or unpinned CLI checks are not pinned-CLI qualification.

The optional real-CDK command-owner check uses the isolated native helper with
`BORROWSER_AG4_COMMAND_SCENARIO=cdk` and `BORROWSER_AG4_TEST_PYTHON` naming the
hash-locked virtual environment; it is still credential-free.

Preparation regressions run the actual script with controlled build helpers for
SIGINT, SIGTERM, polling exceptions, timeout, overflow, failed builds and surviving
descendants. The optional real Docker cancellation check uses
`BORROWSER_AG4_DOCKER_TEST_IMAGE` set to the checked-in builder's `rust@sha256:...`
digest and selects `test_actual_preparation_stops_its_real_container`. Its native
build is a helper; its container lifecycle uses the real local Docker daemon.
This must be reported separately from ordinary helper tests and AWS acceptance.

Guest-wrapper regressions use test-interpreter interposition and pipe handshakes
around actual child acquisition, ordinary capture, TERM grace and publication.
They deliver repeated handled signals and verify reaping, bounded streams,
preserved primary/cleanup diagnostics and absence of successful capture output.
The production wrapper has no fault-injection switches.

Qualify the exact guest wrapper with the intended final-snapshot harness and
pinned Chromium, rather than substituting a direct local `--chromium` invocation:

```sh
BORROWSER_AG4_GUEST_HARNESS=/absolute/final-bundle/borrowser-conformance-linux \
BORROWSER_CHROMIUM_EXECUTABLE=/absolute/pinned/chrome \
python3 -m unittest discover -s crates/conformance/aws/tests \
  -p test_guest_capture.py -k real_pinned -v
```

Run this on the native supported Linux configuration; macOS may additionally
qualify the unchanged wrapper using the controller artifact and macOS Chromium.
Record the actual platform, Python/browser versions and source/artifact hashes.
macOS or emulated-container evidence does not qualify native Linux or AWS.

After pin/configuration review and credential-free safeguards pass, prepare a
fresh final source bundle with `--aws-acceptance`. Record its tree identity,
artifact digest, tool versions, image/snapshot and dates outside deterministic
comparison reports. Opt in explicitly with `BORROWSER_AG4_AWS_ACCEPTANCE=1`,
`BORROWSER_AG4_ACCEPTANCE_BUNDLE`, `BORROWSER_AG4_ACCEPTANCE_CONFIG` and
`BORROWSER_AG4_ACCEPTANCE_STATE`, then run the exact ignored acceptance test.

```sh
cargo test -p borrowser-conformance --features aws-acceptance --locked \
  --bin borrowser-conformance aws::acceptance::real_capture_and_interrupted_create_recovery \
  -- --exact --ignored --nocapture --test-threads=1
```

Use an empty private acceptance state root. This test requests six serial
operations: two identical cascade comparisons, the full registry, accepted
creation followed by lost response, first-interruption cleanup after creation,
and interruption after accepted deletion. Each operation has its own admission
check; six USD-0.50 admission estimates do not constitute an aggregate billing
ceiling. Records remain available after failure. The fault hooks exist only in
an `aws-acceptance` build and are never enabled by ordinary invocation or CI.

Required AWS evidence includes actual cascade and full-registry comparisons,
separate repeated operations, normal terminal cleanup, cancellation cleanup,
deployment/retrieval failures, rollback before root-volume persistence and
cleanup-only recovery after controller interruption. For the accepted-create
case, the acceptance-feature controller's
`BORROWSER_AG4_ACCEPTED_CREATE_CRASH=1` hook exits only after receiving a real
successful creation response, before consuming/saving its stack ARN. Run
cleanup-only without the artifact bundle, prove the unique original stack was
deleted without another creation, and retain the targeted terminal observations.
Also interrupt deletion and repeat cleanup. Missing ownership authority must
refuse deletion. No mocked check substitutes for these live observations.

AG4 is not complete until the actual final source/artifact snapshot has the
required native-platform and AWS acceptance evidence. Tests, documentation,
hardening and qualification remain inside AG4.
