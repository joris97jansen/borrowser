# AG9g0e2a / #1413: identity observations and bounded logical queries

This completes the four identity-service observation adapters and common query
mechanics. It builds on the frozen e1/e1a contracts and the reviewed
[V3 partial identity correction](ag9g0e2a1-partial-identity-observation-v3.md).
It does not rename, migrate or reinterpret historical evidence.

## Ownership and entry boundary

`aws/identity_reads.rs` owns `IdentityObservations` and the closed `IdentityRead`
selector (Caller, Bucket, Profile, Key). Production construction accepts the
reviewed deployment and the explicit session file; it starts the observation clock
before loading credentials. Clients use the existing explicit bounded configuration:
no ambient credentials, proxies or endpoint overrides, one SDK attempt, existing
session/operation timeouts, and disabled S3 Express session authentication.
There are no client accessors or arbitrary-operation/request callbacks. The entire
boundary remains private and is not wired into the controller or CLI.

Observation is not admission. This read boundary cannot construct an admitted
`AwsSession`, authorize a launch or change provider state. Existing
`CallerIdentity::normalize`, `AwsSession::admit_bounded` and bucket admission remain
unchanged. They may reject facts that these independent observers must retain.
Reads never borrow admission normalization or manufacture compliant identifiers.

| Selector | Exact request | Evidence |
| --- | --- | --- |
| Caller | STS GetCallerIdentity, no caller-selected members | V2 Caller, three independent returned members |
| Bucket | S3 HeadBucket with reviewed bucket and reviewed account in ExpectedBucketOwner | V2 Bucket; name/expected_owner are explicitly request provenance, region is returned evidence |
| Profile | IAM GetInstanceProfile with the final name component of the exact reviewed profile ARN; validated IAM name grammar | V3 Profile, independent members and every role occurrence |
| Key | KMS DescribeKey with exact reviewed key ARN; no grant tokens or aliases | V3 Key, independent returned metadata |

Query account/region and exact resource identity are reviewed request provenance.
They never fill an omitted returned member. No scope/discovery selection or required-
read derivation occurs here: the caller explicitly selects one of the four reads
and supplies whether that query is required.

## Returned facts and failures

Completion is not compliance. A successful fully represented contradictory account,
principal, bucket region, profile, role, key metadata or enum literal remains evidence
and can have Complete coverage. No A01–A38 evaluation is performed.

* V2 STS account and S3 region use their existing typed validators. Missing or
  malformed typed members become `Observed::Unavailable(Malformed)`; valid siblings
  remain. Frozen V2 cannot retain a raw malformed typed identifier or distinguish
  omission from malformed input. This limitation is explicit, not a semantic change.
* STS ARN/user ID are bounded literal ProviderText, independent of session-admission
  checks. Explicit empty strings remain Present(empty); omitted members are
  unavailable/malformed. No trimming, sentinel or requested principal is substituted.
* A V2 unavailable/malformed member makes coverage Incomplete(Malformed), while
  retaining the other facts. Overlong text yields a record-bound marker and stops
  the round. NUL is malformed. A bound failure takes precedence over another malformed
  member in the same response.
* IAM/KMS use the reviewed V3 normalizers without changing their bytes or states.
  A bounded Malformed typed member remains explicit evidence and makes coverage
  Incomplete(Malformed), as required by the frozen foundation. Independently valid
  siblings and every role occurrence remain. This differs from a syntactically valid
  contradictory identity, which may have Complete coverage without implying compliance.
  NotReturned, Empty and unknown bounded enum literals alone do not prevent completion.
  Unrepresentable text makes coverage incomplete: TextBytes latches RecordBytes,
  ContainsNul is Malformed. Limit/session latches take precedence. Valid sibling
  evidence remains when the resulting complete record fits its existing ceiling.
* Service/read failures produce no fabricated success record. Access denied, not
  found, session expired, other service failure, malformed decoding and transport
  failure have static closed categories. S3's status-only 403/404 responses are
  classified explicitly. The pinned SDK may wrap malformed HTTP-success JSON in
  an unhandled service error; that case is Malformed, not a service rejection.
* Failed HeadBucket invocations may retain the actual response's single, valid
  `x-amz-bucket-region` header as a V2 Bucket record. This follows the documented
  [301/403 response examples](https://docs.aws.amazon.com/AmazonS3/latest/API/API_HeadBucket.html).
  The pinned SDK's `SdkError::raw_response()` supplies invocation-associated metadata;
  no second request, redirect or retry occurs. Every matching header occurrence is
  checked against the existing 2,048-byte text ceiling before duplicate disposition or
  typed Region parsing. An oversized occurrence in any position latches RecordBytes;
  an existing round failure retains precedence. Traversal borrows only matching values,
  collects no contents and checks the shared round deadline at every occurrence and
  before finalization. When all occurrences fit, duplicate values yield no record and
  preserve the original read failure. Missing, empty or malformed single values also
  yield no record; a 2,048-byte value can fit the representation bound while failing
  the narrower Region grammar. Bucket name and expected_owner remain request
  provenance, never actual ownership. A generic 403 does not identify its precise cause.
  Retained evidence charges one occurrence and canonical bytes through the same shared
  accounting/reserved-coverage path. Coverage stays incomplete and nonterminal; a
  stronger accounting/session failure takes precedence over the service category.
* Service ExpiredToken and local expiry stop the shared round. Existing latched
  transport/time/bound reasons take precedence; Session maps to SessionExpired.
  SDK errors and response text never enter diagnostics or coverage.

IAM presence remains private, structural, bounded and invocation-scoped. The existing
scanner runs before decoding and rejects ampersands in **all** raw attribute values,
including otherwise valid attribute references. It never interprets semantic values.
The SDK owns decoded values, and the consuming receiver rejects incomplete lifecycle,
reuse, correlation disagreement and expiry. A private typed scanner failure carries
the 129th-role bound into the shared Records latch before SDK normalization; it
changes neither accepted XML nor presence semantics. Unsupported structural syntax
remains a malformed read. No bypass or new XML decoder is added.

## Shared execution and accounting

`aws/query_execution.rs` implements `LogicalQuery`, independent of SDK types. It owns
one immutable query, page-attempt accounting, continuation state, retained records and
V1 terminal coverage. It cannot dispatch an operation. Closed adapters drive its
start-page / accept-page / failed-page / finish transitions. The four adapters are
single-page users; deterministic tests drive paginated mechanics without EC2 service
normalization or discovery. There is no generic request closure or arbitrary AWS API.

The operation/scope allowlist is checked separately from frozen QueryIdentityV1
validation. Identity operations accept only Regional caller or a single correctly
typed Bucket/Profile/Key exact identity. Existing EC2 operation names admit only their
explicit typed exact/attribute/offering scopes, with tag/attachment scopes limited
to the existing resource-query operations and client-token scope only for instances.
The allowlist confers no ability to execute EC2 operations. Region/zone and attribute
operations are singleton; token-capable operations use the common continuation rules.

* Only an actual polled BoundedHttp connector call charges a request. A page attempt
  is recorded from that request-counter delta, including service/parser/body failures.
  Preparation failure before a connector call records zero requests and zero pages,
  preserving frozen `pages <= requests`. SDK retries stay disabled.
* A query lease prevents overlapping logical queries on one round. Adapter ownership
  is serial (`&mut self`); SDK clients are encapsulated. Dropping an unfinished query
  latches Cancelled even if cancellation occurs outside the HTTP body polling window.
  A bounded ledger rejects restarting any admitted query identity, including after
  failure or zero transmission; page/cycle state cannot be reset through a fresh
  query object. At most 128 query coverage slots can be admitted. Ledger identity
  bytes are bounded by their already charged coverage reservations. Dropping the
  observing future returns no QueryResult; it cancels the round irrevocably rather
  than returning successful or uncharged replacement evidence. A reported SDK
  cancellation/failure follows ordinary incomplete-coverage finalization.
* Each decoded page charges all semantic occurrences before validating or retaining
  its normalized records. IAM counts the profile response plus every role, including
  duplicate, empty and partial roles. No deduplication or refund occurs. Already
  accepted bounded records survive later failure with explicit incomplete coverage;
  an overflowing record is never truncated or partially stored.
* A successful decoded page with no continuation is terminal, including an empty
  result. Terminal remains true when a later normalization/time check fails; terminal
  alone is insufficient for Complete. Service or decoding failure is not a terminal
  successful page. Stopping with outstanding continuation is always incomplete.
* Continuations must be nonempty, NUL-free and at most 4,096 UTF-8 bytes. They remain
  private and are not logged. Byte overflow latches the existing representation limit,
  RecordBytes, across the round; empty/NUL tokens are query-local Malformed failures.
  Supplied normalization limits are latched on every return path, even when provenance
  validation or continuation processing fails first. Accepted evidence is charged and
  retained before finalizing these failures; the first already-latched reason wins.
  Exact returned states are remembered in a bounded set;
  A→A and A→B→A are PaginationCycle immediately, not deferred page exhaustion.
  Singleton operations reject continuation. At most 16 page attempts are permitted;
  a terminal sixteenth page can complete, but a seventeenth attempt cannot begin.
* Requests, response bytes, normalized bytes, occurrences and time share the original
  round across services and queries. Page accounting is per immutable logical query.
  Limit failures latch the round, so changing query or service cannot reset limits.

## Coverage after exhaustion

Before admitting a query, the executor charges a conservative reservation for its
largest possible terminal ReadCoverageV1 encoding, computed from the closed failure
vocabulary and frozen maximum counters. It uses the existing normalized-byte counter.
The reservation is internal capacity accounting, **not** an emitted observation or
coverage claim. Actual terminal coverage has actual counters and fits that reservation.
Unused reserved bytes are never refunded. Thus accounting may conservatively exceed
retained canonical bytes; retained bytes can never exceed the frozen ceiling.

This avoids trying to allocate terminal evidence after a failure latch or resetting
budgets to do so. Finalization performs the final clock/session check, produces
Complete only for terminal success without failure, and emits deterministic incomplete
coverage otherwise. It never charges twice for coverage or resumes a failed query.

If the round is already unavailable, the scope is unsupported, another query owns the
lease, the identity was already started, the 128 coverage slots are exhausted,
or a coverage reservation cannot fit, `observe` returns a static ReadFailureV1
before admitting the new query and before any request. It cannot fabricate an
unaccounted coverage record for a query that never started. Existing admitted-query
results and their terminal coverage are retained. The caller must stop on a latched
round; no coordinator, query discovery or retry policy is implemented here.

All frozen limits remain: 128 round requests, 16 page attempts/query, 4,096 occurrences,
1 MiB/response, 8 MiB response bytes/round, 16 KiB/canonical record, 256 KiB normalized
round budget, 128 roles, original clock/session limits. Inventory/context/publication
ceilings remain separate and unchanged. Query results are in-memory data, not durable
publication or authenticity proofs.

## Regression evidence and exclusions

`aws::identity_reads::tests` uses the real pinned SDK and synthetic replay transport
for all four adapters: exact input projection, matching and contradictory facts,
omissions/malformed fields, IAM partial occurrences, service/protocol failures,
attribute rejection, record/response bounds and expiry across services.
`aws::query_execution::tests` deterministically checks terminal empty pages, duplicates,
query transitions, cycles, invalid tokens, page/request/record/byte exhaustion,
reserved failure coverage, provenance, cancellation, exclusive ownership and time.
The existing admission, presence, transport and V1/V2/V3 fixture suites remain required.

Validation uses standalone Rust 1.92, locked/offline dependencies and external target
directories on macOS and Linux. No dependency or canonical fixture changes are needed.
Browser smoke tests are not applicable; broader repository CI belongs to integration.

Excluded: EC2 response normalization, discovery coordination, required-read derivation,
A01–A38 admission, candidate selection, reconciliation, inventory construction, durable
publication/replay, controller integration, mutation and cleanup. These exclusions do
not substitute for any of #1413's four adapters or shared logical-query mechanics.
