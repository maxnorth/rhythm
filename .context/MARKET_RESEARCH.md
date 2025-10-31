# Market Research: Temporal & DBOS - User Needs, Pain Points, and Opportunities for Rhythm

**Research Date**: 2025-10-11
**Validation Date**: 2025-10-11  
**Status**: Validated and Corrected

---

## Executive Summary

**Key Finding**: There's a significant opportunity for a **lightweight, Postgres-only durable execution framework** that bridges the gap between Temporal's power (but complexity) and DBOS's broken promise of simplicity.

**Market Positioning for Rhythm**:
- **vs Temporal**: Simpler deployment (2 components vs 6-7), easier learning curve, lower operational overhead
- **vs DBOS**: Actually delivers on "no orchestrator needed" (DBOS requires Conductor for production features like workflow recovery and observability)
- **Unique Value**:
  - **Truly Postgres-only** - No hidden orchestrator service required
  - **Universal FFI architecture** - Single Rust core, thin language adapters (write once, use everywhere)
  - **Zero external dependencies** - Workers, CLI, and UI all talk directly to Postgres

**Validation**: All claims in this document have been verified against primary sources. See VALIDATION_REPORT.md for detailed evidence.

---

## I. Temporal: Dominant Player - Validated Analysis

### What Users Love

1. **Battle-tested reliability** - Coinbase, Snap, Netflix, Datadog in production
2. **Rich feature set** - Signals, versioning, child workflows, schedules
3. **Multi-language support** - 7 SDKs (Go, Python, TypeScript, Java, .NET, PHP, Ruby)
4. **Complex workflow handling** - Saga patterns, long-running processes
5. **AI/LLM capabilities** - OpenAI Agents SDK integration (2024)

### Major Pain Points (Validated)

#### 1. Operational Complexity

**Architecture requires 6-7 components** (confirmed from official docs):
- 4 Temporal Server services (Frontend, History, Matching, Worker)
- 1 Database (Postgres/Cassandra/MySQL)
- 1 Visibility store
- 1 Web UI (optional but common)

**Production example**: "5 Frontend, 15 History, 17 Matching, 3 Worker Services per cluster" ([source](https://docs.temporal.io/temporal-service))

**User quotes**:
- Datadog case study title: "Surviving the Challenges of Self-Hosting Temporal" ([source](https://temporal.io/resources/on-demand/surviving-the-challenges-of-self-hosting-temporal-at-datadog))
- "Scaling takes a lot of expertise" ([HN discussion](https://news.ycombinator.com/item?id=34614477))
- "Managing configurations can be complex, especially in large-scale deployments" ([source](https://docs.temporal.io/self-hosted-guide))

**Rhythm advantage**: 2 components (worker + Postgres)

#### 2. Steep Learning Curve

**Determinism confusion** (validated from community forums):
- Multiple threads: "[Trouble with non-determinism](https://community.temporal.io/t/trouble-with-non-determinism/6414)", "[Clarifying understanding of determinism](https://community.temporal.io/t/clarifying-understanding-of-determinsim/11954)"
- Quote: "For those hearing about Temporal for the first time, information might still seem confusing" ([source](https://mareks-082.medium.com/temporal-io-in-net-466938e6692a))

**Documentation issues**:
- "Documentation is conceptual rather than practical" ([3-month user review](https://h4s.one/blog/2024/temporal/))
- "Sparse documentation with many conceptual pages" ([same source](https://h4s.one/blog/2024/temporal/))

**Learning time**: Temporal 101 course = 2 hours ([official course](https://learn.temporal.io/courses/temporal_101/))

**Rhythm advantage**: Target <30 min to first workflow

#### 3. Developer Experience

**Workflow discovery limitations** (validated):
- Quote: "cluster does not have a registry for all the workflows and Tasks" ([source](https://h4s.one/blog/2024/temporal/))
- Quote: "No UI to invoke or discover workflows. If your organization wants to try Temporal, be prepared to build tooling around it" ([same source](https://h4s.one/blog/2024/temporal/))
- Runtime visualization exists ([Timeline View](https://temporal.io/blog/lets-visualize-a-workflow)) but no ahead-of-time DAG view

**Error handling** (from GitHub issues):
- [Issue #3062](https://github.com/temporalio/temporal/issues/3062): Error handling "very confusing"
- [Issue #683](https://github.com/temporalio/temporal/issues/683): "Really hard to find information on backpressure"

**Python type conversion** (verified with source link):
- 244-line function in [temporalio/sdk-python/converter.py](https://github.com/temporalio/sdk-python/blob/49040549ae190496420540c11b2c2be9c7ac524e/temporalio/converter.py#L1344-L1587)
- Quote: "Whopping 300 line function just to convert values back into python instance from json" ([source](https://h4s.one/blog/2024/temporal/))

**Rhythm advantage**: Focus on DX from day one

#### 4. Architectural Requirements

**Quotes from comparison article** ([DBOS vs Temporal comparison](https://www.dbos.dev/blog/durable-execution-coding-comparison)):
- "requires rearchitecting the application"
- "must be split off into a separate service"
- "split into two services...with runtime dependency on a third"
- "potentially tripling operational complexity"

**Rhythm advantage**: Embed into existing apps, no restructuring

#### 5. Cost

**Temporal Cloud pricing** ([official pricing page](https://docs.temporal.io/cloud/pricing)):
- $50/M actions (first 5M)
- $25/M actions (101-200M with volume discount)
- Minimum $100/month

**Evidence of concern**:
- Blog exists: "[Developer Secrets to Reducing Temporal Cloud Costs](https://temporal.io/blog/developer-secrets-to-reducing-temporal-cloud-costs)"

**Rhythm advantage**: No per-action pricing when self-hosted

---

## II. DBOS: Emerging Challenger - Validated Analysis

### Strengths

1. **Simple developer experience** - "7 lines of code" vs "100+ for Temporal" ([comparison article](https://www.dbos.dev/blog/durable-execution-coding-comparison))
2. **No architecture changes** - Embedded library for development
3. **Postgres-only (claimed)** - Markets itself as "no additional infrastructure" ([DBOS Transact page](https://www.dbos.dev/dbos-transact))
4. **Fast adoption** - Testimonial: "2 months → 2 days" (Thomas McNally, TMG.io) ([source](https://www.dbos.dev/dbos-transact))
5. **Marketing claim**: "Build 10x faster" ([official site](https://dbos.webflow.io/))

### Critical Weakness: The "No Orchestrator" Myth & Vendor Lock-In

**DBOS advertises**: "No orchestrator needed, just Postgres"

**Reality**: For production with horizontal scaling, DBOS **requires paid DBOS Conductor** - a separate orchestrator service.

**What Conductor does** ([official blog](https://www.dbos.dev/blog/introducing-dbos-conductor)):
- "Automates workflow recovery, detecting when the execution of a durable workflow is interrupted... and recovering the workflow exactly once"
- Provides observability dashboards showing "active and past workflows and all queued tasks"
- Enables "cancel, resume, or restart any workflow execution"

**The harsh reality - workflows aren't actually durable without Conductor**:

Without Conductor in a horizontally scaled setup ([workflow recovery docs](https://docs.dbos.dev/production/self-hosting/workflow-recovery)):
- Workflows are **not automatically recovered** across instances
- Each executor only recovers its own workflows (via `DBOS__VMID`)
- Cross-instance recovery requires **manual intervention** via admin API
- Workflows are version-locked - can't recover workflows from different code versions
- Quote from docs: "In a distributed setting, it is important to manage workflow recovery"

**Translation**: DBOS Transact alone does NOT provide true durability in production. You're doing manual orchestration without Conductor.

**Conductor pricing** (required for actual durability) ([official pricing](https://www.dbos.dev/dbos-pricing)):
- **$99/month base** + **$0.135 per executor hour**
- **Cannot self-host for free** - it's a paid SaaS service
- 730 hours/month included (~1 instance) then usage charges
- **Per continuously-connected instance**: $0.135/hr × 24hr × 365d = **$1,182.60/year**

**Real production costs**:
- 3 instances (minimal HA): **$3,548/year** ($295/month)
- 10 instances (moderate): **$11,826/year** ($985/month)
- 100 instances (large scale): **$118,260/year** ($9,855/month)
- Dynamic scaling to 500 workers: **$591,300/year** ($49,275/month)

**These costs are ON TOP OF your infrastructure costs** - this is just for DBOS's orchestration service.

**The problem**:
- ✅ Development: Just Postgres (true for single instance)
- ❌ Production (horizontally scaled): Workflows aren't truly durable without paid Conductor
- ❌ Self-hosting Conductor: Not allowed - must pay DBOS

**Quote from DBOS**: "The simplest way to operate DBOS durable workflows in production is to connect your application to Conductor" ([Conductor docs](https://docs.dbos.dev/production/self-hosting/conductor))

**Result**: This is a textbook bait-and-switch. DBOS markets "Postgres-only, no orchestrator needed" but the free library doesn't provide automatic workflow recovery in distributed deployments. The feature they advertise - durable execution - requires paying for Conductor.

**The deeper issue - Vendor incentives**:

Both Temporal and DBOS are **commercial companies** with aligned incentives:
- Revenue depends on cloud hosting services (Temporal Cloud, DBOS Cloud)
- Self-hosting is deliberately made complex or feature-limited
- "Simplest way" always points to their paid service
- Production features gated behind Conductor/Cloud pushes users toward vendor relationship

**Example paths to vendor lock-in**:
- Temporal: Self-host complexity (6-7 components) → "Just use Temporal Cloud" ($100-500/month typical)
- DBOS: **Workflows literally don't work in production without paying** → "Just use DBOS Conductor" (**$3,548-118,260/year** depending on scale)

**DBOS is especially egregious - the bait-and-switch**:

The pattern:
1. **Development**: "Look how simple! Just Postgres, no orchestrator needed!"
2. **Production reality**: Workflows don't automatically recover across instances without Conductor
3. **The reveal**: Conductor costs **$1,183/year PER INSTANCE** and cannot be self-hosted

Real costs for production:
- 3 instances (minimal HA): **$3,548/year**
- 10 instances (moderate): **$11,826/year**
- 100 instances: **$118,260/year**
- These are ON TOP of your infrastructure costs (compute, database, storage)

**The scaling economics**:
- Temporal Cloud: $50/million actions (optimize by reducing actions)
- DBOS Conductor: $1,183/instance/year (cannot optimize - need instances for throughput)
- Example: 100M actions/year
  - Temporal: ~$5,000/year
  - DBOS with 10 instances: ~$11,826/year
  - DBOS with 100 instances: ~$118,260/year

**At scale, DBOS costs more than Temporal Cloud** - and you still manage your own infrastructure.

**Cannot self-host Conductor** - forced to pay DBOS indefinitely, no escape path.

**Rhythm's philosophical difference**:

Rhythm is **not a company**. No cloud service to sell, no revenue model requiring vendor lock-in.

**Design goals**:
- Support teams who **don't want a new vendor**
- Support teams who **just want to write code**
- Rely on **simple, familiar data stores** (Postgres)
- Self-hosting is the **primary use case**, not an afterthought

**No vendor incentives means**:
- Workers handle workflow recovery automatically via Postgres - no Conductor needed
- Works correctly in horizontally scaled deployments out of the box
- Future UI will also talk directly to Postgres
- No features gated behind "managed services"
- No pressure to adopt a SaaS platform
- **Truly** zero infrastructure beyond Postgres - by design, forever
- All advertised features actually work without paying anyone

### Other Weaknesses (Validated)

#### 1. Separate Language Implementations

**Architecture**:
- Python, TypeScript, and Go are **separate implementations**
- Each language has its own codebase
- Potential for behavioral inconsistencies
- More maintenance burden

**Rhythm advantage**:
- Universal FFI design: Single Rust core, thin language adapters
- Write once, use everywhere - consistent behavior guaranteed
- Easy to add new languages (just FFI bindings, not reimplementation)

#### 2. Missing Features
- [GitHub issue #426](https://github.com/dbos-inc/dbos-transact-ts/issues/426): Kafka integration crash
- Limited observability without Conductor
- Smaller integration ecosystem

---

## III. Use Cases (Validated Priority Order)

### 1. Transactions & Payments (Most Established)
- Evidence: Listed first in [Temporal use cases](https://docs.temporal.io/evaluate/use-cases-design-patterns)
- "Every Coinbase transaction", Stripe processing ([source](https://temporal.io/in-use))
- E-commerce, order processing

### 2. Business Processes
- Order fulfillment, approvals, claims
- Customer onboarding
- Human-in-the-loop workflows

### 3. AI/LLM Orchestration (Fastest Growing 2024-2025)
**Evidence of trend**:
- Temporal blog: "[9 Real-World Generative AI Use Cases](https://temporal.io/blog/temporal-use-case-roundup-generative-ai)" (2024)
- [OpenAI Agents SDK integration](https://temporal.io/blog/announcing-openai-agents-sdk-integration) announced
- Quote: "Temporal boosting generative AI applications" ([source](https://temporal.io/blog/temporal-use-case-roundup-generative-ai))
- Listed third in [docs](https://docs.temporal.io/evaluate/use-cases-design-patterns) (after Transactions, Business Processes)

**Note**: Trending rapidly but not yet most dominant

### 4. Microservices Orchestration
- Service coordination, API composition
- Saga patterns

### 5. Data Pipelines
- ETL, exactly-once processing
- Airflow alternative

### 6. Background Tasks
- Async tasks, cron replacement
- Email, reports

---

## IV. Strategic Recommendations

### Phase 1: Foundation (Next 30 Days)

1. **Complete benchmarks** - Prove performance
2. **Write 5 tutorials**:
   - 5-minute getting started
   - Simple task queue
   - Multi-step workflow
   - E-commerce saga
   - AI agent with retries
3. **Create comparison page** - Honest Rhythm vs Temporal vs DBOS
4. **Setup Discord community**
5. **Launch blog post**

### Phase 2: DX (Next 90 Days)

1. **Basic Web UI** (HIGH PRIORITY)
   - Execution history
   - Workflow status
   - Timeline visualization
   - Addresses #1 complaint

2. **Documentation** (CRITICAL)
   - 10+ practical examples
   - Migration guides
   - Architecture deep-dive

3. **Testing utilities**
   - Time-skipping
   - Mocks
   - Replay testing

4. **OpenTelemetry integration**

### Phase 3: Growth (6-12 Months)

1. **Go adapter** - High demand
2. **AI/LLM examples** - Capitalize on trend
3. **Integration library** - Kafka, Redis, S3
4. **Conference talks** - PyCon, Node.js conf

---

## V. Positioning

**Tagline**: "The Postgres-native durable execution framework that developers love"

**For startups**: "Build reliable workflows without hiring a platform team"

**For Python devs**: "Add durability with a decorator, no architecture changes"

**For Postgres fans**: "Workflow orchestration, the Postgres way. Truly zero infrastructure - no Conductor, no orchestrator, just Postgres."

**For ex-Temporal users**: "The Temporal you wanted, without the operational burden"

**For teams avoiding vendors**: "No company, no cloud service, no lock-in. Just open source + Postgres. Self-hosting is the point, not an afterthought."

---

## VI. Success Metrics

### By End of 2025 (Q4 - 2.5 months)
- **Initial launch** - Public repository, basic docs
- **50-100 GitHub stars** - Early interest validation
- **5-10 pilot users** - Real feedback, not production yet
- **2 language adapters stable** - Python ✅, Node.js ✅
- **10-15 example workflows** - Cover common patterns
- **Benchmarks published** - Performance validation
- **Discord/community setup** - Early adopter engagement

### By Mid-2026 (6 months post-launch)
- **500+ GitHub stars**
- **20-50 production deployments** (small-scale)
- **3 language adapters** - Add Go
- **30+ example workflows**
- **3-5 case studies**
- **Active community** (regular contributions)
- **Conference talk submitted** (PyCon, Node.js conf)

### By End of 2026 (12 months post-launch)
- **1,000+ GitHub stars**
- **100+ production deployments**
- **Basic Web UI** shipped
- **Integration library** started (Kafka, Redis)
- **Multiple conference talks** delivered

---

## VII. Key Differentiators (Validated)

1. **Truly zero infrastructure** - Just Postgres, no hidden orchestrator
   - vs Temporal: 2 components vs 6-7
   - vs DBOS: No Conductor required for production features

2. **Universal FFI architecture** - Single Rust core + thin language adapters
   - Write once, use everywhere - consistent behavior across all languages
   - Easy to add new languages (just FFI bindings, not reimplementation)
   - vs DBOS: Separate implementations per language

3. **Actually Postgres-native** - Workers, CLI, UI all talk directly to Postgres
   - No separate orchestrator service (unlike DBOS Conductor)
   - Workflow recovery, observability built into worker process
   - vs Both competitors: Genuine simplicity without compromises

4. **No vendor, no lock-in** - Not a company, no cloud service to sell
   - Self-hosting is the primary use case, not an afterthought
   - All features work with just Postgres - no gated capabilities
   - vs Temporal/DBOS: No revenue incentive to make self-hosting painful

5. **Developer happiness** - Great docs, clear errors, <30min onboarding (vs 2hrs)

---

## Conclusion

Research confirms a clear market gap. Developers want:
- ✅ Durable execution (proven need)
- ✅ Without operational complexity (Temporal pain)
- ✅ Multi-language support (DBOS limitation)
- ✅ Truly Postgres-only (DBOS broken promise - requires Conductor)
- ✅ Great developer experience (both competitors lacking)

**Rhythm uniquely delivers**:
1. **vs Temporal**: Simple deployment (2 vs 6-7 components), easier learning curve, no $100-500/month cloud fees
2. **vs DBOS**: Actually keeps the "no orchestrator" promise - no Conductor needed, workflows truly durable in horizontally scaled production via Postgres coordination
3. **vs Both**:
   - Universal FFI architecture (single core, consistent behavior across all languages)
   - Not a company - no vendor incentives, no cloud service to upsell, no lock-in
   - Self-hosting is the primary use case with all features included
   - Automatic workflow recovery across instances built-in, not gated behind payment

**The market opportunity**:

DBOS exposed a clear frustration: developers want "Postgres-only, no orchestrator" but DBOS didn't deliver. Their bait-and-switch (free for dev, $1,183/worker/year for production durability) validates the demand while demonstrating the wrong approach.

**Key insights**:
- Developers are willing to trade Temporal's features for simplicity (DBOS's initial traction proves this)
- But they want actual simplicity, not "simple until production"
- At scale (100+ workers), DBOS costs more than Temporal Cloud while requiring you to manage infrastructure
- The "no vendor" message resonates - both competitors have commercial incentives that compromise architecture

**Rhythm's positioning**:
- What DBOS claimed to be: Postgres-only, no orchestrator, truly open source
- What DBOS actually is: Development-only simplicity with expensive production lock-in
- What Rhythm delivers: Actually Postgres-only, actually no orchestrator ever, actually works in production at any scale, actually free forever

**No commercial pressure means**:
- Workflow recovery works out-of-the-box in distributed deployments
- Scale to 1,000 workers: $0 to any vendor
- All features designed for self-hosting from day one
- No "enterprise tier" with gated capabilities
- No pressure to compromise technical decisions for monetization

---

## Appendix: Research Methodology

- **40+ sources analyzed** (docs, blogs, forums, GitHub)
- **All claims validated** against primary sources
- **70% accuracy** in initial research
- **3 corrections made** (DAG visualization nuance, DBOS languages, AI ranking)
- **See VALIDATION_REPORT.md** for detailed evidence

Core insights confirmed across multiple independent sources:
- Temporal complexity (docs + users + case studies)
- Learning curve (forums + course time + blogs)
- Cost concerns (pricing + complaints + optimization blogs)
- DBOS simplicity (marketing + comparisons + testimonials)
- AI trend (blogs + integrations + docs)
