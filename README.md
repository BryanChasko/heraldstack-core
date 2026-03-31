```
  +-[ heraldstack ]----------------------------------------+
  |  ambient ai infrastructure for bryan chasko            |
  |  shannon . haunting . gander . ibeji                   |
  +---------------------------------------------------------+
```

multi-platform agent collective across claude code, kiro, goose, and gemini cli.
four agent platforms, one shared knowledge backbone (qdrant + valkey), full
opentelemetry tracing, rust-first application logic, aws serverless deployment.

---

## critical: no new shell scripts for application logic

bias is rust for all new functionality. do not create shell scripts for application logic. instead:

- add features to existing rust binaries
- update documentation
- add --help flags to existing tools for self-documenting usage

shell scripts that stay as shell:
- deployment scripts orchestrating external tools (aws cli, docker)
- check-rust.sh (must work even when rust code has issues)
- ci/cd pipeline scripts for infrastructure tasks

---

## automated tools -- run these before manual fixes

```bash
./target/release/check_json --fix        # json formatting
./scripts/validation/check-rust.sh       # rust formatting, clippy, tests
./target/release/format_md               # markdown formatting
./target/release/validate_naming --fix   # naming convention checks
```

---

## development standards

- [development principles](docs/DEVELOPMENT-PRINCIPLES.md)
- [naming conventions](docs/naming-conventions.md)
- build + deploy: `./scripts/deploy/deploy.sh`
- json tools: rust-based utilities in `src/utils/json-tools`
- all ingestion follows [modular ingest refactor plan](docs/migration/INGEST-MIGRATION-MODULAR-PLAN.md)

---

## architecture

all interactions flow through harold, who routes context to specialized agents.
four cli platforms share knowledge via qdrant collections (copywriting:8100,
writing-inbox:8101, shared-knowledge:8102) and four expansion collections
(prompt-transcripts, agent-memory, shannon-methodology, verbal-ticks) on port 8103.

```
shannon (claude code)  -- code, architecture, ci/cd
haunting (kiro)        -- research, knowledge base, document analysis
gander (goose)         -- automation, pipeline execution
ibeji (gemini)         -- context bridging, multi-modal
```

---

## documentation

- [jsonl format for vector embedding](docs/vector-search/jsonl-ingestion.md)
- [migration documentation](docs/migration/)
- [modular ingest refactor plan](docs/migration/INGEST-MIGRATION-MODULAR-PLAN.md)
- [ethics guidelines](config/ethics/LawsOfRobotics.json)

---

style guide: https://github.com/BryanChasko/heraldstack-mcp/blob/main/STYLE_GUIDE.md

mit license 2025 bryan chasko
