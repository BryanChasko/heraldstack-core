---
document_type: "Architecture Gameplan"
version: "0.1.0"
status: "DRAFT — pending matrix amendment + GitHub org confirmation"
last_review: "2026-05-15"
owner: "poltergeist-stratia-aws-infra"
cross_links:
  - chasko-labs/heraldstack-core (current)
  - heraldstack/heraldstack-nova-forge (pending creation)
  - splintercells-deep-agents-cli (URL TBD)
---

> **DRAFT** — This document is not final. Two things unblock promotion to APPROVED:
>
> 1. Capability-matrix amendment lands (adds `ts` to `allowed_file_types`, adds `heraldstack-nova-forge` to `allowed_repos` for `poltergeist-stratia-aws-infra`)
> 2. GitHub org access confirmed — issue filing against `heraldstack/haunting-kiro-cli` succeeds (gh auth currently fails; all `ISSUE_*` placeholders below are unfiled)

## Table of Contents

- [1. Overview](#1-overview)
- [2. Repo Layout](#2-repo-layout)
- [3. 4-Stack Architecture](#3-4-stack-architecture)
- [4. Mandatory Tag Policy via CDK Aspects](#4-mandatory-tag-policy-via-cdk-aspects)
- [5. cdk-nag Baseline](#5-cdk-nag-baseline)
- [6. SSM Kill Switch](#6-ssm-kill-switch)
- [7. Woodpecker CI Pipeline](#7-woodpecker-ci-pipeline)
- [8. Deploy Sequence](#8-deploy-sequence)
- [9. Cross-Links](#9-cross-links)
- [10. Open Issues](#10-open-issues)

---

## 1. Overview

`heraldstack-nova-forge` is the AWS infrastructure repository for heraldstack. It owns all CDK-managed cloud resources in AWS account **946179428633** — VPC topology, stateful data stores, compute workloads, observability plumbing. Single source of truth for IaC in the heraldstack AWS footprint.

**Relationship to splintercells-deep-agents-cli:** nova-forge provisions the AWS substrate that splintercells agents consume at runtime — IAM roles, SSM parameters, ECS task definitions, Lambda ARNs. The CLI is the invocation layer; nova-forge is the ground it runs on. Cross-link URL is TBD pending `BryanChasko/haunting-kiro-cli#443` (see §10).

**Scope boundary:** nova-forge does not manage Bedrock model configuration (owned by `poltergeist-stratia-bedrock-arch`) or agent governance rules (owned by `haunting-kiro-cli`). It manages the AWS resources those layers depend on.

---

## 2. Repo Layout

```
heraldstack-nova-forge/
├── bin/
│   └── app.ts                        # CDK App entry — instantiates all stacks
├── lib/
│   ├── network-stack.ts              # NovaForgeNetworkStack
│   ├── data-stack.ts                 # NovaForgeDataStack
│   ├── compute-stack.ts              # NovaForgeComputeStack
│   ├── observability-stack.ts        # NovaForgeObservabilityStack
│   ├── aspects/
│   │   └── required-tags-aspect.ts   # Fail-closed tag enforcement IAspect
│   └── nag-suppressions/
│       ├── network-suppressions.ts
│       ├── data-suppressions.ts
│       ├── compute-suppressions.ts
│       └── observability-suppressions.ts
├── test/
│   ├── network-stack.test.ts
│   ├── data-stack.test.ts
│   ├── compute-stack.test.ts
│   └── observability-stack.test.ts
├── docs/
│   ├── architecture.md               # Stack dependency diagram + decision log
│   ├── runbook.md                    # Deploy, rollback, kill-switch ops
│   └── open-issues.md                # Mirrors §10; updated per release
├── .woodpecker/
│   ├── deploy.yaml                   # synth → diff → deploy (push to main)
│   ├── pr-check.yaml                 # synth → diff → nag report (pull_request)
│   └── drift.yaml                    # cdk drift --fail (cron: @daily)
├── cdk.json
├── package.json
├── tsconfig.json
└── README.md
```

Notes:

- `bin/app.ts` is the only file that instantiates stacks. Cross-stack dependencies wired via constructor props — never `Fn::ImportValue`.
- `lib/aspects/` and `lib/nag-suppressions/` are separate directories. Enforcement logic stays out of stack files.
- `test/` uses Jest. Each stack has a corresponding test file asserting resource presence and tag compliance.
- `docs/open-issues.md` mirrors §10; updated as issues close.

---

## 3. 4-Stack Architecture

Deploy ordering is explicit, enforced by CDK dependency graph via constructor prop wiring. Stacks split by deployment lifecycle and blast radius per [CDK Best Practices](https://docs.aws.amazon.com/cdk/v2/guide/best-practices.html).

### Deploy Order

```
1. NovaForgeNetworkStack
2. NovaForgeDataStack        (depends on: Network)
3. NovaForgeComputeStack     (depends on: Network, Data)
4. NovaForgeObservabilityStack (depends on: Compute)
```

---

### Stack 1: NovaForgeNetworkStack

Foundational network layer. Rarely changes; all other stacks depend on it.

**Resources:**

- VPC (multi-AZ, private + public subnets)
- NAT Gateways
- Security Groups (baseline ingress/egress)
- VPC Endpoints (SSM, S3, ECR — reduces NAT traffic, satisfies AwsSolutions-VPC7)

**Cross-stack refs:** Exports `vpc` and `securityGroups` as construct properties. No `CfnOutput` for internal consumers — outputs reserved for external systems.

**Removal policy:** `RETAIN` on VPC. Termination protection: OFF (network is redeployable; stateful resources live in Data).

---

### Stack 2: NovaForgeDataStack

All stateful resources. Deployed independently from compute — schema migrations don't touch workloads.

**Resources:**

- DynamoDB tables (on-demand; point-in-time recovery enabled)
- S3 buckets (versioning enabled; lifecycle rules for archival)
- ElastiCache cluster (deferred to first compute requirement)
- SSM Parameter: `/heraldstack/nova-forge/<env>/kill-switch` (see §6)

**Cross-stack refs:** Exports `table`, `bucket`, `killSwitchParam` as construct properties. Kill switch parameter ARN passed to Compute for scoped `ssm:GetParameter` grants.

**Removal policy:** `RETAIN` on all stateful resources. Termination protection: **ON**. Only stack with termination protection enabled.

---

### Stack 3: NovaForgeComputeStack

Stateless workloads. Deploys frequently; depends on Network and Data outputs.

**Resources:**

- Lambda functions (with Parameters and Secrets Extension for kill-switch reads)
- ECS Cluster + Fargate task definitions
- API Gateway (REST or HTTP — decided at first service definition)
- ALB (if ECS services require HTTP routing)
- IAM roles scoped per workload (no wildcard resource policies)

**Cross-stack refs:** Receives `vpc`, `securityGroups` from Network; `table`, `bucket`, `killSwitchParam` from Data. Exports `ecsCluster`, `lambdaFunctions` for Observability.

**Removal policy:** `DESTROY` on Lambda and ECS (stateless, safe to recreate). `RETAIN` on IAM roles referenced by external trust policies.

---

### Stack 4: NovaForgeObservabilityStack

Monitoring, alerting, tracing. Reads outputs from all other stacks; deploys independently.

**Resources:**

- CloudWatch Dashboards (per-stack operational views)
- CloudWatch Alarms (Lambda error rate, ECS task failures, DynamoDB throttles)
- Log Groups (explicit retention — no indefinite retention)
- X-Ray tracing groups
- SNS Topics for alarm routing

**Cross-stack refs:** Receives `ecsCluster`, `lambdaFunctions` from Compute. Uses `CfnOutput` for dashboard URLs (external consumers). No SSM lookups — all refs wired at synth time.

**Removal policy:** `DESTROY` on dashboards and alarms. `RETAIN` on Log Groups (audit trail).

---

### bin/app.ts wiring

```typescript
import { App, Environment } from "aws-cdk-lib";
import { NovaForgeNetworkStack } from "../lib/network-stack";
import { NovaForgeDataStack } from "../lib/data-stack";
import { NovaForgeComputeStack } from "../lib/compute-stack";
import { NovaForgeObservabilityStack } from "../lib/observability-stack";
import { RequiredTagsAspect } from "../lib/aspects/required-tags-aspect";
import { AwsSolutionsChecks } from "cdk-nag";
import { Aspects, Tags } from "aws-cdk-lib";

const app = new App();
const env: Environment = { account: "946179428633", region: "us-east-1" };
const stage = app.node.tryGetContext("stage") ?? "dev";

Tags.of(app).add("Owner", "heraldstack");
Tags.of(app).add("Project", "nova-forge");
Tags.of(app).add("Environment", stage);
Tags.of(app).add("CostCenter", "engineering");
Tags.of(app).add("ManagedBy", "cdk");

const network = new NovaForgeNetworkStack(app, "NovaForgeNetwork", { env });
const data = new NovaForgeDataStack(app, "NovaForgeData", {
  env,
  vpc: network.vpc,
  stage,
});
const compute = new NovaForgeComputeStack(app, "NovaForgeCompute", {
  env,
  vpc: network.vpc,
  securityGroups: network.securityGroups,
  table: data.table,
  bucket: data.bucket,
  killSwitchParam: data.killSwitchParam,
});
new NovaForgeObservabilityStack(app, "NovaForgeObservability", {
  env,
  ecsCluster: compute.ecsCluster,
  lambdaFunctions: compute.lambdaFunctions,
});

Aspects.of(app).add(new RequiredTagsAspect());
Aspects.of(app).add(new AwsSolutionsChecks({ verbose: true }));
```

---

## 4. Mandatory Tag Policy via CDK Aspects

**Required tag keys:** `Owner`, `Project`, `Environment`, `CostCenter`, `ManagedBy`

Enforcement uses `Annotations.of(node).addError()` inside an `IAspect` visitor. `addError` causes `cdk synth` to exit non-zero — fail-closed gate. Pattern validated at scale by GoDaddy ([AWS DevOps Blog, April 2025](https://aws.amazon.com/blogs/devops/streamlining-cloud-compliance-at-godaddy-using-cdk-aspects/)).

```typescript
// lib/aspects/required-tags-aspect.ts
import { IAspect, Annotations, CfnResource } from "aws-cdk-lib";
import { IConstruct } from "constructs";

const REQUIRED_TAGS = [
  "Owner",
  "Project",
  "Environment",
  "CostCenter",
  "ManagedBy",
] as const;

export class RequiredTagsAspect implements IAspect {
  visit(node: IConstruct): void {
    if (!(node instanceof CfnResource)) return;
    if (!("tags" in node)) return;
    const present = (node as any).tags?.tagValues() ?? {};
    for (const key of REQUIRED_TAGS) {
      if (!present[key]) {
        Annotations.of(node).addError(
          `[nova-forge] Missing required tag "${key}" on ${(node as CfnResource).cfnResourceType} at ${node.node.path}`,
        );
      }
    }
  }
}
```

**Enforcement gate:** `cdk synth --strict` in CI. `--strict` promotes warnings to errors; combined with `addError`, any untagged resource fails synthesis before a CloudFormation template is emitted.

**Default tag application:** `Tags.of(app).add(...)` in `bin/app.ts` applies all five required tags before the Aspect runs. The Aspect is a safety net for resources that bypass app-level tag propagation (custom resources, CDK-internal bootstrap resources).

---

## 5. cdk-nag Baseline

`AwsSolutionsChecks` applied at App level in `bin/app.ts`. Full AWS Solutions rule pack runs against every stack at synth time. ([cdk-nag](https://github.com/cdklabs/cdk-nag), [AWS DevOps Blog](https://aws.amazon.com/de/blogs/devops/manage-application-security-and-compliance-with-the-aws-cloud-development-kit-and-cdk-nag/))

**Suppression policy:**

Every suppression carries:

1. `id` — exact cdk-nag rule ID (e.g., `AwsSolutions-S1`)
2. `reason` — human-readable justification
3. Applied at the most specific construct path available

Suppressions live in `lib/nag-suppressions/<stack>-suppressions.ts`. Each file exports a single function called after stack construction. Keeps suppression rationale co-located with the stack it covers.

```typescript
// lib/nag-suppressions/compute-suppressions.ts
import { NagSuppressions } from "cdk-nag";
import { Stack } from "aws-cdk-lib";

export function applyComputeStackSuppressions(stack: Stack): void {
  NagSuppressions.addStackSuppressions(stack, [
    {
      id: "AwsSolutions-L1",
      reason:
        "Lambda runtime pinned to project standard (Node 20); upgrade tracked separately",
    },
  ]);
}
```

**Error-level suppression prevention:** `SuppressionIgnoreErrors` prevents suppression of Error-level nag rules. Only Warning-level rules are suppressible.

```typescript
Aspects.of(app).add(
  new AwsSolutionsChecks({
    verbose: true,
    suppressionIgnoreCondition: new SuppressionIgnoreErrors(),
  }),
);
```

**Audit cadence:** Every suppression reviewed at each release. Release checklist includes: "review `lib/nag-suppressions/` — confirm all reasons remain accurate; remove suppressions for resolved findings."

---

## 6. SSM Kill Switch

**Parameter path:** `/heraldstack/nova-forge/<env>/kill-switch`

- Default value: `enabled`
- Kill value: `disabled`
- Type: `String` (not `SecureString` — value is intentionally observable)

**Location:** `StringParameter` construct lives in **NovaForgeDataStack**. Correct home: stateful configuration resource, Data stack has termination protection ON — prevents accidental deletion during compute redeployments.

### Lambda read pattern

Uses the [AWS Parameters and Secrets Lambda Extension](https://docs.aws.amazon.com/systems-manager/latest/userguide/ps-integration-lambda-extensions.html) (localhost:2773 cache, `SSM_PARAMETER_STORE_TTL=60`). 60-second TTL: kill switch takes effect within one minute.

```typescript
async function isEnabled(): Promise<boolean> {
  const url =
    `http://localhost:2773/systemsmanager/parameters/get` +
    `?name=${encodeURIComponent(process.env.KILL_SWITCH_PARAM!)}&withDecryption=false`;
  const res = await fetch(url, {
    headers: {
      "X-Aws-Parameters-Secrets-Token": process.env.AWS_SESSION_TOKEN!,
    },
  });
  const body = (await res.json()) as { Parameter: { Value: string } };
  return body.Parameter.Value !== "disabled";
}

export const handler = async (event: unknown) => {
  if (!(await isEnabled())) {
    console.log("Kill switch active — skipping execution");
    return { statusCode: 200, body: "service suspended" };
  }
  // normal execution
};
```

### ECS read pattern

ECS tasks receive kill switch value at cold start via `ecs.Secret.fromSsmParameter()`. Long-running tasks poll SSM SDK on a 60-second timer and drain gracefully when `disabled` is detected.

```typescript
taskDefinition.addContainer("Worker", {
  secrets: {
    KILL_SWITCH: ecs.Secret.fromSsmParameter(props.killSwitchParam),
  },
});
```

### IAM permissions

Kill switch parameter ARN passed from DataStack to ComputeStack as construct prop. Each execution role gets a scoped grant:

```typescript
props.killSwitchParam.grantRead(workerLambda);
props.killSwitchParam.grantRead(ecsTaskRole);
```

`grantRead` emits `ssm:GetParameter` scoped to the exact parameter ARN. No wildcard. Satisfies AwsSolutions-IAM5.

### Escalation behavior

When `disabled`:

- **Lambda:** returns `{ statusCode: 200, body: 'service suspended' }` — no error, no retry storm
- **ECS long-running:** finish current work unit, skip next iteration, log state
- **ECS scheduled:** check at startup; if disabled, exit 0 (not exit 1 — no alarm trigger)

The kill switch is a graceful drain, not an emergency stop. For hard stops: ECS service desired count = 0 or Lambda reserved concurrency = 0.

**Operational commands:**

```bash
# Disable
aws ssm put-parameter --name /heraldstack/nova-forge/prod/kill-switch --value disabled --overwrite

# Re-enable
aws ssm put-parameter --name /heraldstack/nova-forge/prod/kill-switch --value enabled --overwrite
```

---

## 7. Woodpecker CI Pipeline

Three files in `.woodpecker/`. Three concerns. ([Woodpecker Workflow Syntax](https://woodpecker-ci.org/docs/usage/workflow-syntax))

### .woodpecker/pr-check.yaml

```yaml
when:
  event: pull_request

steps:
  - name: install
    image: node:20-alpine
    commands: [npm ci]

  - name: synth
    image: node:20-alpine
    commands: [npx cdk synth --strict]
    environment:
      AWS_REGION: { from_secret: aws_region }
      AWS_ROLE_ARN: { from_secret: ci_role_arn }
      TRUST_ANCHOR_ARN: { from_secret: trust_anchor_arn }
      PROFILE_ARN: { from_secret: profile_arn }

  - name: diff
    image: node:20-alpine
    commands: [npx cdk diff --fail]
    environment:
      AWS_REGION: { from_secret: aws_region }
```

### .woodpecker/deploy.yaml

```yaml
when:
  event: push
  branch: main

steps:
  - name: install
    image: node:20-alpine
    commands: [npm ci]

  - name: synth
    image: node:20-alpine
    commands: [npx cdk synth --strict]
    environment:
      AWS_REGION: { from_secret: aws_region }
      TRUST_ANCHOR_ARN: { from_secret: trust_anchor_arn }
      PROFILE_ARN: { from_secret: profile_arn }
      AWS_ROLE_ARN: { from_secret: ci_role_arn }

  - name: deploy
    image: node:20-alpine
    commands:
      - npx cdk deploy --all --require-approval never
    environment:
      AWS_REGION: { from_secret: aws_region }
      TRUST_ANCHOR_ARN: { from_secret: trust_anchor_arn }
      PROFILE_ARN: { from_secret: profile_arn }
      AWS_ROLE_ARN: { from_secret: ci_role_arn }
```

### .woodpecker/drift.yaml

```yaml
when:
  event: cron
  cron: drift-check

steps:
  - name: detect-drift
    image: node:20-alpine
    commands:
      - npm ci
      - npx cdk drift --fail
    environment:
      AWS_REGION: { from_secret: aws_region }
      TRUST_ANCHOR_ARN: { from_secret: trust_anchor_arn }
      PROFILE_ARN: { from_secret: profile_arn }
      AWS_ROLE_ARN: { from_secret: ci_role_arn }

  - name: notify
    image: alpine
    commands: [echo "Drift detected — alert via SNS/webhook"]
    when:
      status: [failure]
```

### AWS credential strategy

**Deferred to lyra dispatch — see `chasko-labs/heraldstack-core#77`.**

Woodpecker v3.14 does not natively issue OIDC tokens. Correct keyless pattern: **IAM Roles Anywhere** with X.509 certificates issued by a private CA ([AWS Security Blog](https://aws.amazon.com/it/blogs/security/enable-external-pipeline-deployments-to-aws-cloud-by-using-iam-roles-anywhere/)). Pipeline steps reference `trust_anchor_arn`, `profile_arn`, `ci_role_arn` as Woodpecker secrets. `aws_signing_helper credential-process` exchanges the X.509 cert for temporary STS credentials. Long-lived static credentials are not acceptable.

---

## 8. Deploy Sequence

### Step 1: Bootstrap

```bash
npx cdk bootstrap aws://946179428633/us-east-1 \
  --cloudformation-execution-policies arn:aws:iam::aws:policy/AdministratorAccess
```

Runs once per account/region. Creates CDKToolkit stack with S3 staging bucket and ECR repository.

### Step 2: Deploy stacks in dependency order

```bash
npx cdk deploy --all --require-approval never
```

CDK resolves order from dependency graph. For targeted redeployments, use explicit stack names:

```bash
npx cdk deploy NovaForgeNetwork --require-approval never
npx cdk deploy NovaForgeData --require-approval never
npx cdk deploy NovaForgeCompute --require-approval never
npx cdk deploy NovaForgeObservability --require-approval never
```

### Step 3: Kill-switch verification

Kill switch parameter created by NovaForgeDataStack (Step 2). Verify:

```bash
aws ssm get-parameter \
  --name /heraldstack/nova-forge/dev/kill-switch \
  --query Parameter.Value \
  --output text
# Expected: enabled
```

### Step 4: Smoke verify

```bash
aws ec2 describe-vpcs --filters "Name=tag:Project,Values=nova-forge" --query 'Vpcs[].VpcId'
aws ssm get-parameter --name /heraldstack/nova-forge/dev/kill-switch --query Parameter.Value
aws lambda list-functions --query 'Functions[?starts_with(FunctionName, `NovaForge`)].FunctionName'
```

### Step 5: Confirm termination protection on Data stack

```bash
aws cloudformation update-termination-protection \
  --stack-name NovaForgeData \
  --enable-termination-protection
```

Idempotent. CDK's `terminationProtection: true` handles this automatically — this step is manual verification.

---

## 9. Cross-Links

### splintercells-deep-agents-cli

URL: TBD per `BryanChasko/haunting-kiro-cli#443`

splintercells-deep-agents-cli is the agent invocation layer consuming AWS resources provisioned by nova-forge:

- IAM roles for agent execution (NovaForgeComputeStack)
- SSM parameters for runtime configuration (NovaForgeDataStack)
- ECS task definitions for long-running agent workloads (NovaForgeComputeStack)

When `BryanChasko/haunting-kiro-cli#443` closes, substitute the real GitHub URL here and in `docs/architecture.md`.

### haunting-kiro-cli

Repository: `heraldstack/haunting-kiro-cli`

haunting-kiro-cli is the agent governance layer — poltergeist personas, capability matrices, dispatch rules, session management. nova-forge is the AWS substrate that haunting-managed agents run on.

The bridge: capability-matrix amendment tracked in `BryanChasko/haunting-kiro-cli#445`. Until that closes, `poltergeist-stratia-aws-infra` cannot be dispatched to author `.ts` files in nova-forge (matrix row lacks `ts` in `allowed_file_types` and `heraldstack-nova-forge` in `allowed_repos`).

**Dependency chain:**

```
BryanChasko/haunting-kiro-cli#445 closes
  → poltergeist-stratia-aws-infra gains write surface on nova-forge
  → nova-forge .kiro/steering/ initialized
  → first live CDK-TS dispatch proceeds
```

---

## 10. Open Issues

> **Note:** Issue filing aborted — `gh` could not reach `heraldstack/haunting-kiro-cli` (auth/access failure). All entries below are unfiled placeholders. Re-dispatch `file_issues` stage after resolving org access.

| Key                                 | Repo Target       | Description                                                                                                                                                   | Unblocks                                                                                       |
| ----------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `BryanChasko/haunting-kiro-cli#443` | haunting-kiro-cli | Establish canonical GitHub URL for `splintercells-deep-agents-cli`; document nova-forge ↔ CLI dependency contract                                             | Cross-link URLs in §1 and §9 finalized                                                         |
| `chasko-labs/heraldstack-core#77`   | haunting-kiro-cli | Dispatch to `poltergeist-lyra-aws-identity`: decide IAM Roles Anywhere (X.509) vs OIDC for Woodpecker CI credentials in 946179428633                          | §7 pipeline secrets finalized; CI credential setup proceeds                                    |
| `BryanChasko/haunting-kiro-cli#445` | haunting-kiro-cli | PR to capability-matrix: add `ts` to `allowed_file_types`, add `heraldstack-nova-forge` to `allowed_repos`, correct model drift, add CDK-TS line to DUTIES.md | `poltergeist-stratia-aws-infra` gains authorized write surface; first CDK-TS dispatch proceeds |
| `ISSUE_NOVA_FORGE_BOOTSTRAP`        | haunting-kiro-cli | Track CDK bootstrap execution for 946179428633/us-east-1 — confirm toolkit stack exists, staging bucket accessible from CI role                               | §8 Step 1 marked complete; stack deploys proceed                                               |
| `ISSUE_NOVA_FORGE_STEERING`         | heraldstack-core  | Initialize `heraldstack-nova-forge/.kiro/steering/` with CDK-TS repo-type steering                                                                            | typeverify passes for nova-forge; pre-dispatch checklist cleared                               |

---

## References

- [AWS CDK v2 Best Practices](https://docs.aws.amazon.com/cdk/v2/guide/best-practices.html)
- [AWS CDK Aspects](https://docs.aws.amazon.com/cdk/v2/guide/aspects.html)
- [AWS CDK Multiple Stacks](https://docs.aws.amazon.com/cdk/v2/guide/stack-how-to-create-multiple-stacks.html)
- [cdk-nag](https://github.com/cdklabs/cdk-nag/blob/main/README.md)
- [GoDaddy CDK Aspects — AWS DevOps Blog](https://aws.amazon.com/blogs/devops/streamlining-cloud-compliance-at-godaddy-using-cdk-aspects/)
- [AWS Parameters and Secrets Lambda Extension](https://docs.aws.amazon.com/systems-manager/latest/userguide/ps-integration-lambda-extensions.html)
- [IAM Roles Anywhere — AWS Security Blog](https://aws.amazon.com/it/blogs/security/enable-external-pipeline-deployments-to-aws-cloud-by-using-iam-roles-anywhere/)
- [CDK CLI `cdk drift`](https://docs.aws.amazon.com/cdk/v2/guide/ref-cli-cmd-drift.html)
- [Woodpecker CI Workflow Syntax](https://woodpecker-ci.org/docs/usage/workflow-syntax)
- [Woodpecker CI Cron](https://woodpecker-ci.org/docs/usage/cron)
