# Deploying to Azure

This guide walks through provisioning the registry's infrastructure on Azure
using the Azure Developer CLI (`azd`). The deployment uses
[infra/main.bicep](../infra/main.bicep) to create a resource group, Log
Analytics workspace, Container Apps environment, two Container Apps
(frontend + backend), and an Azure Database for PostgreSQL Flexible Server.

> **Infrastructure or release?** Use
> [`just provision`](#provision-infrastructure-without-a-release) to update
> infrastructure while keeping your chosen container images. The numbered
> steps below describe the underlying manual `azd` flow. To publish and deploy
> a new release from CI — including GitHub deployment status tracking — see
> [Automated deployment via GitHub Actions](#automated-deployment-via-github-actions).

## Provision infrastructure without a release

Run these commands from the checkout whose infrastructure you intend to apply.
Use your existing deployment checkout when possible: a fresh worktree does not
automatically have its production `.azure/` configuration.

```sh
# Sign in explicitly if needed; the helper never changes authentication for you.
az login
azd auth login

just provision                         # use the selected azd environment
just provision existing-environment    # explicitly select an environment for this run

# Equivalent commands using the repository's existing Rust task runner:
cargo xtask provision
cargo xtask provision existing-environment
```

The command requires the repository's **Rust toolchain/Cargo**, `az`, and
**azd 1.25+** with `azd env set --file` support. `just` is optional: the recipe
is a thin wrapper around `cargo xtask provision`. Cargo builds the task runner
on its first invocation; Python, Docker, and GitHub CLI are not needed.
The [Rust provisioning helper](../crates/xtask/src/provision/mod.rs) checks
configuration and then runs `azd provision --environment <name> --no-prompt`
against the existing [azure.yaml](../azure.yaml) and Bicep. It does **not**
build or push application images, publish crates, run a release, create cloud
identities, grant permissions, or configure GitHub secrets.

### Environment selection and missing setup

An explicit recipe argument selects the environment for that invocation without
changing an existing default. Otherwise the helper uses `AZURE_ENV_NAME` from
the process, then azd's selected default. If there is no unambiguous selection,
it asks rather than choosing an arbitrary environment.

When the named local environment is missing, the helper offers guided setup
and asks before creating it. **For an existing deployment, recover the original
environment configuration first whenever possible.** You must retain its
subscription, region, environment name, resource-group override, database
login/name/password, domain, and any custom settings. Guessing these can
create different resources or change credentials. Remote-only azd environments
must be restored locally explicitly; this helper does not initialize remote
state stores.

Saved settings are preserved. Process variables can fill missing values, but a
conflict with saved settings stops the command and names the conflicting keys
without showing their values. Unset the conflicting input or deliberately
change the azd setting before retrying. This includes an exported
`AZURE_ENV_NAME` that disagrees with an explicit environment argument.
Optional settings that remain unset
continue to use the Bicep defaults, including scaling and logging parameters;
the helper does not reset them.

Missing required settings are prompted for. The current Azure CLI subscription
can be offered as a default, but the helper never switches subscriptions.
If the selected deployment targets a different subscription, it stops with an
explicit `az account set` instruction so the provisioning hooks cannot
accidentally operate on the wrong account.

### Images and credentials

Both `BACKEND_IMAGE` and `FRONTEND_IMAGE` must identify the application images
you intend to deploy. For an infrastructure-only update, retain their currently
deployed references. There is no automatic version lookup, `latest` fallback,
or demonstration image substitution. Explicitly configured `latest` or
untagged references are preserved only after an additional warning and
confirmation; prefer a version tag or digest to avoid pulling different code
when a new revision starts.

Public images, including public GHCR packages, do not require registry
credentials. For private access, supply the matching `REGISTRY_SERVER`,
`REGISTRY_USERNAME`, and `REGISTRY_PASSWORD`. The helper asks about visibility
when setting up missing images and prompts only for genuinely missing private
registry settings. It does not automatically use `GHCR_PULL_TOKEN`, which is
the separate CI deployment workflow's secret.

Missing passwords are read with **terminal echo disabled**. For existing
PostgreSQL resources, enter the **original administrator password**, not a new
one: the Bicep deployment re-applies it. No password or token is generated,
rotated, or fetched from GitHub. Do not pass secrets as recipe arguments or
paste them into shell-history commands.

After confirmation, missing settings are imported using azd's file-input
interface, not command-line values. The temporary import file is owner-only
on Unix and is removed on success, failure, or interruption. The resulting
`.azure/<environment>/.env` is **gitignored plaintext, not encrypted storage**;
the helper restricts its Unix permissions when importing settings. Protect the
directory and its backups with your operating system's permissions. Supported
process environment inputs also contain plaintext and should come from a
trusted secret-loading mechanism. The helper never prints an environment dump
and redacts known credentials from subprocess output.

Some azd versions, including 1.25.5, cannot reliably preserve dotenv values
ending in a backslash or double quote. The helper conservatively rejects those
values before saving or provisioning, rather than corrupting a credential.
Keep the original value and resolve the azd storage compatibility issue; do
not rotate a production password merely to get past this check.

### Confirmation and outcome

Before any provision, the helper shows the non-secret target and image
references and requires an explicit confirmation. Declining does not save
entered settings or start provisioning. This is an interactive command:
missing input or confirmation without a terminal fails rather than assuming
consent. Azure still validates permissions, image access, and resource changes;
passing local checks is not a guarantee that deployment will succeed.

Provisioning uses the existing provider-registration and custom-domain hooks.
Read their warnings and verify the website and `/v1/health` endpoint afterward:
deferred domain/certificate bindings do not necessarily make azd exit with an
error. A failed or interrupted deployment can leave partial Azure changes;
cancellation is not rollback.

There is no separate preview recipe or preview environment. Dry-run support is
deferred. In particular, raw `azd provision --preview` still invokes project
hooks in the supported azd version; the existing hooks can register providers
or update domain bindings, so that command is **not** a guaranteed no-write
alternative here.

`just release` remains the full publish-and-deploy workflow.
[`scripts/setup-azure-deploy.sh`](../scripts/setup-azure-deploy.sh) is separate
CI setup that **writes GitHub secrets and variables**; `just provision` never
calls it.

The helper's Rust command-flow tests run offline with strict CLI doubles:

```sh
cargo test --package xtask provision
```

They also run as part of the existing `cargo xtask test` CI checks.

## Prerequisites

Install the following tools:

- [Azure Developer CLI (`azd`)](https://learn.microsoft.com/azure/developer/azure-developer-cli/install-azd) — v1.25 or newer
- [Azure CLI (`az`)](https://learn.microsoft.com/cli/azure/install-azure-cli)
- [Docker](https://docs.docker.com/get-docker/) — only needed when manually
  building new container images, not for provisioning existing images

You also need an Azure subscription where you have **Owner** or
**Contributor + User Access Administrator** rights (required to create
resource groups and register resource providers).

## 1. Sign in

Sign into both CLIs against the same tenant and subscription:

```sh
az login
az account set --subscription <SUBSCRIPTION_ID_OR_NAME>

azd auth login
```

Verify:

```sh
az account show --query "{name:name, id:id, tenant:tenantId}" -o table
```

## 2. Create or select an `azd` environment

Each `azd` environment is a named bag of configuration (location, env name,
secrets) stored under `.azure/<env-name>/`. The environment name is also used
to derive Azure resource names — e.g. an environment called `wasm-registry`
produces a resource group `rg-wasm-registry`, a Container Apps environment
`cae-wasm-registry`, and so on (see [infra/main.bicep](../infra/main.bicep)).

Create a new environment (the first time only):

```sh
azd env new wasm-registry
```

Or select an existing one:

```sh
azd env select wasm-registry
azd env list
```

## 3. Set required environment variables

The deployment reads the following keys from the azd environment store. They
are wired into Bicep through [infra/main.bicepparam](../infra/main.bicepparam)
via `readEnvironmentVariable(...)`.

| Variable                  | Required | Description                                                                                          |
| ------------------------- | -------- | ---------------------------------------------------------------------------------------------------- |
| `AZURE_ENV_NAME`          | yes      | Logical environment name. Drives resource-group and resource naming. Set automatically by `azd env new`. |
| `AZURE_LOCATION`          | yes      | Azure region (e.g. `centralus`, `westus3`). See note on region restrictions below.                   |
| `AZURE_SUBSCRIPTION_ID`   | yes      | Target subscription ID. Set it explicitly with `azd env set` (see below).                            |
| `POSTGRES_ADMIN_PASSWORD` | yes      | Postgres admin password. Min 8 chars, must include upper, lower, digit, and symbol.                  |
| `BACKEND_IMAGE`           | no       | Backend container image. Defaults to a placeholder. Set to a ghcr.io image for real deployments.     |
| `FRONTEND_IMAGE`          | no       | Frontend container image. Defaults to a placeholder. Set to a ghcr.io image for real deployments.    |
| `AZURE_RESOURCE_GROUP`    | no       | Override the default resource group name (`rg-${AZURE_ENV_NAME}`).                                   |
| `POSTGRES_ADMIN_LOGIN`    | no       | Postgres admin user. Defaults to `pgadmin`.                                                          |
| `POSTGRES_DB`             | no       | Postgres database name. Defaults to `componentregistry`.                                             |
| `CUSTOM_DOMAIN_NAME`      | no       | Apex domain to serve the frontend on (e.g. `wasm.directory`). When set, provisioning also creates a DNS zone with records for both the apex (frontend) and the `api.` subdomain (meta-registry API). See [Bind a custom domain](#7-optional-bind-a-custom-domain). |
| `LOG_ANALYTICS_DAILY_QUOTA_GB` | no | Maximum Log Analytics ingestion per day, in GB. Defaults to `1`; ingestion stops for the remainder of the day when exceeded. |
| `LOG_ANALYTICS_RETENTION_IN_DAYS` | no | Number of days to retain Log Analytics data. Defaults to `30`, which is both the platform minimum and the amount included free on the `PerGB2018` SKU. Values below `30` are rejected during deployment validation, before any resources are created; values above it are billed as extended retention. |
| `BACKEND_MIN_REPLICAS`    | no       | Lower bound on backend replicas. Defaults to `1`, keeping the API always on. Accepts `0`–`10`; `0` scales to zero when idle, at the cost of a cold start. |
| `BACKEND_MAX_REPLICAS`    | no       | Upper bound on backend replicas, and therefore on worst-case backend compute spend. Defaults to `1`. Accepts `1`–`10`; raised to match `BACKEND_MIN_REPLICAS` if that is set higher. |
| `BACKEND_CONCURRENT_REQUESTS` | no   | In-flight HTTP requests per backend replica before another is added. Defaults to `10`, matching the platform's implicit default. Accepts `1`–`1000`; raising it on a 0.25 vCPU container risks a rule that never fires, see [Cost](#cost). |
| `FRONTEND_MIN_REPLICAS`   | no       | Lower bound on frontend replicas. Defaults to `1`, keeping the site always on. Accepts `0`–`10`; `0` scales to zero when idle, at the cost of a cold start. |
| `FRONTEND_MAX_REPLICAS`   | no       | Upper bound on frontend replicas, and therefore on worst-case frontend compute spend. Defaults to `1`. Accepts `1`–`10`; raised to match `FRONTEND_MIN_REPLICAS` if that is set higher. |
| `FRONTEND_CONCURRENT_REQUESTS` | no  | In-flight HTTP requests per frontend replica before another is added. Defaults to `10`, matching the platform's implicit default. Accepts `1`–`1000`; the frontend renders server-side on the same 0.25 vCPU, so see [Cost](#cost) before raising it. |

Set them with `azd env set`:

```sh
azd env set AZURE_SUBSCRIPTION_ID '<your-subscription-id>'
azd env set AZURE_LOCATION centralus
azd env set BACKEND_IMAGE 'ghcr.io/<owner>/component-cli/backend:latest'
azd env set FRONTEND_IMAGE 'ghcr.io/<owner>/component-cli/frontend:latest'
```

Use `just provision` for hidden entry of a missing `POSTGRES_ADMIN_PASSWORD`
and confirmation before applying. Restore the original password when updating
an existing deployment; do not generate a replacement.

Inspect individual non-secret settings rather than dumping credentials:

```sh
azd env get-value AZURE_ENV_NAME
azd env get-value AZURE_LOCATION
azd env get-value BACKEND_IMAGE
azd env get-value FRONTEND_IMAGE
```

> **Tip:** `POSTGRES_ADMIN_PASSWORD` is written in plain text to
> `.azure/<env>/.env`. That file is gitignored, but treat it as a secret on
> disk. For CI, inject it from a secret store at run time.

### Region restrictions

Some subscriptions (notably MSDN / Visual Studio benefit subscriptions) are
blocked from provisioning PostgreSQL Flexible Servers in certain regions,
producing errors like `LocationIsOfferRestricted` or
`NoRegisteredProviderFound`. To probe which regions are open to your
subscription:

```sh
for r in eastus centralus westus3 northcentralus canadacentral; do
  reason=$(az postgres flexible-server list-skus --location "$r" -o json 2>/dev/null \
    | python3 -c "import sys,json; d=json.load(sys.stdin); print('OK' if d[0].get('supportedServerEditions') else 'RESTRICTED')")
  printf '  %-20s %s\n' "$r" "$reason"
done
```

Pick any region that prints `OK` and set it with
`azd env set AZURE_LOCATION <region>`.

## 4. Provision the infrastructure

```sh
azd provision
```

What happens:

1. The `preprovision` hook ([infra/hooks/preprovision.sh](../infra/hooks/preprovision.sh))
   registers required resource providers (`Microsoft.App`,
   `Microsoft.OperationalInsights`, `Microsoft.ContainerRegistry`,
   `Microsoft.DBforPostgreSQL`, `Microsoft.Insights`, `Microsoft.Network`)
   and waits for them to reach `Registered`.
2. `azd` runs the subscription-scoped deployment defined in
   [infra/main.bicep](../infra/main.bicep), which creates the resource group
   and then deploys the resources composed in
   [infra/resources.bicep](../infra/resources.bicep).
3. Outputs (resource group, FQDNs, Container Apps environment ID) are
   written back into the `azd` environment store and surfaced in the
   console.

First-time provider registration on a fresh subscription can take 10–15
minutes for `Microsoft.App`. Subsequent deploys reuse the existing
registration and skip the wait.

## 5. Build and push container images

Container images are built locally with Docker and pushed to GitHub
Container Registry (`ghcr.io`). The images must be public (or you must
add registry credentials to the Container Apps configuration).

```sh
# Authenticate to ghcr.io
echo $GITHUB_TOKEN | docker login ghcr.io -u <USERNAME> --password-stdin

# Build and push backend
docker build -f Dockerfile.backend -t ghcr.io/<OWNER>/component-cli/backend:latest --platform linux/amd64 .
docker push ghcr.io/<OWNER>/component-cli/backend:latest

# Build and push frontend (API_BASE_URL is baked in at compile time)
docker build -f Dockerfile.frontend -t ghcr.io/<OWNER>/component-cli/frontend:latest \
  --platform linux/amd64 --build-arg API_BASE_URL=http://backend .
docker push ghcr.io/<OWNER>/component-cli/frontend:latest
```

Then point `azd` at the published images:

```sh
azd env set BACKEND_IMAGE 'ghcr.io/<OWNER>/component-cli/backend:latest'
azd env set FRONTEND_IMAGE 'ghcr.io/<OWNER>/component-cli/frontend:latest'
```

## 6. Deploy

Run `azd provision` (or re-run it if you already provisioned the
infrastructure). It picks up `BACKEND_IMAGE` and `FRONTEND_IMAGE` from
the environment and deploys the Container Apps with those images:

```sh
azd provision
```

The service URLs are printed at the end. You can also retrieve them later:

```sh
azd env get-values | grep _URL
```

## 7. (Optional) Bind a custom domain

By default the frontend is reachable only on its generated
`*.azurecontainerapps.io` URL. To serve it on an apex domain you own (for
example `wasm.directory`), set `CUSTOM_DOMAIN_NAME` before provisioning:

```sh
azd env set CUSTOM_DOMAIN_NAME wasm.directory
azd provision
```

With the variable set, `azd provision` also deploys
[infra/modules/dns.bicep](../infra/modules/dns.bicep), which creates a public
DNS zone for the domain with:

- an apex `A` record pointing at the Container Apps environment's static
  ingress IP (the frontend website), and an `asuid` `TXT` record carrying the
  frontend's domain-verification id;
- an `api` `A` record pointing at the same static ingress IP (the
  meta-registry API — the backend is provisioned with external ingress so it
  is reachable independently of the frontend), and an `asuid.api` `TXT` record
  carrying the backend's domain-verification id.

Both verification records let Azure validate ownership before issuing the
managed certificates. The `component` CLI targets the API host
(`https://api.wasm.directory`) by default, so binding the `api` subdomain is
what makes `component registry notify` (and `sync`/`search`/`install`/`run`)
work against production.

Binding has two parts — the first is manual (only you can do it), the second
is automated by `azd provision`:

1. **Delegate the zone.** Read the name servers Azure assigned and point your
   registrar's `NS` records at exactly that set:

   ```sh
   azd env get-value DNS_NAME_SERVERS
   ```

   Then wait for propagation (`dig +short NS wasm.directory` should return the
   Azure name servers; `dig +short TXT asuid.wasm.directory` and
   `dig +short TXT asuid.api.wasm.directory` should return the frontend and
   backend verification ids respectively).

2. **Add the hostnames + bind their certificates (automated).** After every
   `azd provision`, the `postprovision` hook runs
   [`scripts/bind-custom-domains.sh`](../scripts/bind-custom-domains.sh)
   (`scripts/bind-custom-domains.ps1` on Windows). It is idempotent and:

   - adds the apex hostname to the frontend and the `api` hostname to the
     backend (ownership validated against the `asuid` / `asuid.api` `TXT`
     records);
   - issues and binds the free managed TLS certificates using **HTTP**
     validation for both hostnames — temporarily allowing plain HTTP on each
     app so DigiCert's probe can reach it, then restoring each app's original
     ingress setting. (Container Apps managed-cert `TXT` validation for the
     `api` subdomain proved unreliable here — certificates got stuck in
     `Pending` — so both use HTTP validation.)
   - verifies each endpoint over HTTPS.

   Because delegation (step 1) can only happen after the zone exists, the
   **first** `azd provision` reports the binds as *deferred* and prints exactly
   what to delegate. Once delegation has propagated, finish the bind by
   re-running either the provision or the script directly. The script reads the
   apex domain from `CUSTOM_DOMAIN_NAME` in the azd environment; pass it as the
   first argument when running from a checkout whose azd environment does not
   have it set (a fresh clone, or a CI-managed environment):

   ```sh
   ./scripts/bind-custom-domains.sh                  # domain from the azd env
   ./scripts/bind-custom-domains.sh wasm.directory   # or pass it explicitly
   # Windows: pwsh ./scripts/bind-custom-domains.ps1 -Domain wasm.directory
   # equivalently: azd env set CUSTOM_DOMAIN_NAME wasm.directory && azd provision
   ```

   On success it verifies and reports both endpoints:

   ```
   ==> Custom domains bound: https://wasm.directory and https://api.wasm.directory
   ```

   The certificates auto-renew afterwards. If you prefer to bind by hand — or
   want to see exactly what the hook does — the underlying `az` commands are:

   <details>
   <summary>Manual bind commands</summary>

   ```sh
   RG="$(azd env get-value AZURE_RESOURCE_GROUP)"
   ENVNAME="$(azd env get-value AZURE_CONTAINER_APPS_ENVIRONMENT_NAME)"
   APP="$(azd env get-value SERVICE_FRONTEND_NAME)"
   DOMAIN="$(azd env get-value CUSTOM_DOMAIN_NAME)"
   API_APP="$(azd env get-value SERVICE_BACKEND_NAME)"
   API_DOMAIN="$(azd env get-value CUSTOM_API_DOMAIN_NAME)"   # e.g. api.wasm.directory

   # Frontend (apex): add the hostname (validated against the asuid TXT record;
   # a bind without adding first fails with RequireCustomHostnameInEnvironment),
   # then bind over HTTP validation. Apex certs validate over HTTP, not TXT (TXT
   # is only for subdomains and leaves an apex cert stuck in Pending; see
   # https://learn.microsoft.com/azure/container-apps/custom-domains-managed-certificates#free-certificate-requirements).
   # The frontend redirects HTTP->HTTPS, which blocks DigiCert's probe, so allow
   # insecure traffic for the bind and restore the redirect afterwards.
   az containerapp hostname add \
     --resource-group "$RG" --name "$APP" --hostname "$DOMAIN"
   az containerapp ingress update -n "$APP" -g "$RG" --allow-insecure true
   az containerapp hostname bind \
     --resource-group "$RG" --name "$APP" --hostname "$DOMAIN" \
     --environment "$ENVNAME" --validation-method HTTP
   az containerapp ingress update -n "$APP" -g "$RG" --allow-insecure false

   # Backend (api subdomain): the docs say subdomains can validate over the
   # asuid.api TXT record, but managed-cert TXT validation proved unreliable
   # here (the cert stayed stuck in Pending), so bind over HTTP instead. Open
   # plain HTTP for DigiCert's probe. Unlike the apex, the backend keeps
   # allowInsecure=true afterwards so the in-environment frontend can keep
   # reaching http://backend (see infra/modules/backend.bicep) — nothing to
   # restore here.
   az containerapp hostname add \
     --resource-group "$RG" --name "$API_APP" --hostname "$API_DOMAIN"
   az containerapp ingress update -n "$API_APP" -g "$RG" --allow-insecure true
   az containerapp hostname bind \
     --resource-group "$RG" --name "$API_APP" --hostname "$API_DOMAIN" \
     --environment "$ENVNAME" --validation-method HTTP

   # Verify
   curl -sS -o /dev/null -w '%{http_code}\n' "https://$DOMAIN/"               # expect 200
   curl -sS -o /dev/null -w '%{http_code}\n' "https://$API_DOMAIN/v1/health"  # expect 200
   ```

   </details>

After the binds complete, the site is at `https://wasm.directory` and the
meta-registry API (what the CLI uses) is at `https://api.wasm.directory`.

## 8. Tear down

To delete everything provisioned by this template:

```sh
azd down --purge --force
```

`--purge` also removes soft-deleted resources (Key Vault, Log Analytics)
so the same `AZURE_ENV_NAME` can be reused immediately.

## Cost

Both apps run one replica at 0.25 vCPU / 0.5 GiB with `minReplicas` and
`maxReplicas` both set to `1`, so the bill is flat at roughly **$62/month**:
about $22 per always-on replica, about $17 for the `Standard_B1ms` PostgreSQL
server and its minimum storage, about $1 for Azure DNS, and under $1 for normal
Log Analytics ingestion. Because the two replica bounds are equal there is no
gap between the expected bill and the worst case — spend cannot rise without a
configuration change.

The trade-off is that neither app has burst headroom, and a single replica is
also a single point of failure while it restarts. Both bounds are environment
variables, so raising them needs no code change.

### What the billing data showed

An earlier revision of this guide quoted about $36/month while assuming **one
replica per app**. Billing data confirms the per-unit rates above, but over one
observed multi-day period both apps sustained three replicas — the `maxReplicas`
of the day — putting measured spend at about $216/month, roughly 92% of it
Container Apps compute billed at the *active* rate.

That plateau is **not** explained by measured load, and its cause is
unresolved. Over the same period the backend averaged 0.83 requests/second at
about 95 ms per request, or roughly 0.08 concurrent requests — about two orders
of magnitude below the ~10-concurrent threshold Container Apps applies by
default when no `scale.rules` entry is declared. Scale-in does work: when
traffic later dropped roughly tenfold, both apps fell to one replica within
hours, with no latency regression. Long-lived connections inflating the
scaler's concurrency count, or bursts hidden inside the averaging window, are
plausible explanations, but neither is confirmed.

Do not read $216/month as a current run-rate, or the figure above as a measured
$216 → $62 saving. That measurement came from the plateau period at roughly ten
times the traffic seen since, and replica counts fell on their own when traffic
dropped. What the current settings buy is a lower cost per replica and a hard
ceiling, not a like-for-like reduction. Replica count, not per-replica sizing,
remains the dominant cost term — verify it with the `Replicas` metric before
trusting any estimate.

### How scaling is configured

Both apps declare an explicit `http-scaling` rule rather than inheriting the
platform's implicit ~10-concurrent-request default. This does not resolve the
plateau above; it makes the behaviour visible in the template and tunable per
environment.

- `BACKEND_CONCURRENT_REQUESTS` defaults to `10`, matching the previously
  implicit default. Raising it is tempting for a low-traffic service, but the
  backend runs on 0.25 vCPU, where CPU saturates well before ten simultaneous
  requests — a higher value risks a rule that never fires when a burst needs it.
- `FRONTEND_CONCURRENT_REQUESTS` also defaults to `10`. The frontend is not a
  static site: it renders every page server-side and calls the backend per
  request, so on the same 0.25 vCPU it saturates on CPU well before ten
  in-flight requests. Neither threshold has been load-tested.
- `BACKEND_MIN_REPLICAS` and `FRONTEND_MIN_REPLICAS` default to `1` to keep both
  services always on. The frontend previously scaled to zero, which was cheaper
  but made the first visitor after an idle period wait on a cold start.
- `BACKEND_MAX_REPLICAS` and `FRONTEND_MAX_REPLICAS` default to `1`, down from a
  ceiling of 3. One replica served the observed load at about 95 ms with no
  latency regression. With both apps at 0.25 vCPU, that lowers the provisioned
  ceiling from 1.5 vCPU to 0.5 vCPU. A minimum set above its maximum would be
  an invalid scale block, so each template raises the maximum to match rather
  than failing at deploy time.

The backend resource reduction has not been load-verified. If the service shows
memory pressure or latency regressions, revert the backend `resources` block
first. The Log Analytics `dailyQuotaGb` setting is deliberately a cost guard:
if it is exceeded, log ingestion stops for the remainder of that day rather
than indicating a workspace fault. Retention is not a cost lever here: the
`PerGB2018` SKU includes 30 days at no extra charge and rejects anything
shorter, so ingestion volume — not `LOG_ANALYTICS_RETENTION_IN_DAYS` — is what
drives the Log Analytics bill.

## Automated deployment via GitHub Actions

The [`Release` workflow](../.github/workflows/release.yml) deploys to Azure
automatically. After its `publish-images` job builds and pushes the `backend`
and `frontend` images to GHCR, the `deploy` job rolls them out to the Container
Apps and records the rollout through the [GitHub Deployments API][deployments]
(visible under the repository's **Environments → production** tab).

The `deploy` job runs `azd up` — the same entry point as a manual deploy —
against [azure.yaml](../azure.yaml) + [infra/main.bicep](../infra/main.bicep),
seeding the parameters that [infra/main.bicepparam](../infra/main.bicepparam)
reads via `readEnvironmentVariable(...)` into an ephemeral azd environment with
`azd env set`, so the Bicep template stays the single source of truth and
secrets never touch the command line. Because `azure.yaml` declares no
`services:`, the deploy phase is a no-op and only the infrastructure is
provisioned, pointed at the images that were just published and pinned to the
exact released `:X.Y.Z` tag (not `:latest`). Provisioning is incremental, so it
keeps the existing resource group and applies only what changed. The frontend
URL is reported back as the deployment's `environment_url`.

azd reuses the Azure CLI's OIDC session (`azd config set auth.useAzCliAuth
true`), which the `preprovision` hook also depends on since it registers the
required resource providers through `az`.

Deploys are **not gated**: every successful `Release` run deploys to production.

### One-time setup

The workflow assumes the infrastructure already exists and that an Entra
identity is available for GitHub to authenticate as. Do this once:

1. **Provision the infrastructure.** Follow sections 1–6 above so the resource
   group, Container Apps environment, Postgres server, and the two Container
   Apps exist. The `deploy` job self-heals drift on later runs, but the first
   provision uses the interactive `azd`/`az` flow.

2. **Create an OIDC identity for GitHub Actions.** Register an Entra
   application and add a **federated credential** so GitHub can sign in without
   a stored secret. Because `release.yml` is triggered by `workflow_dispatch`,
   the OIDC token's subject is the branch ref, so scope the credential to the
   branch you release from:

   ```sh
   # Create the app registration and a service principal for it.
   appId=$(az ad app create --display-name "component-registry-gha-deploy" \
     --query appId -o tsv)
   az ad sp create --id "$appId"

   # Federated credential: GitHub OIDC, subject = the release branch.
   az ad app federated-credential create --id "$appId" --parameters '{
     "name": "github-actions-release-main",
     "issuer": "https://token.actions.githubusercontent.com",
     "subject": "repo:yoshuawuyts/component-registry:ref:refs/heads/main",
     "audiences": ["api://AzureADTokenExchange"]
   }'
   ```

   > If you later add an approval gate by giving the `deploy` job a
   > `environment: production`, switch the subject to
   > `repo:yoshuawuyts/component-registry:environment:production`.

3. **Grant the identity access.** `infra/main.bicep` is subscription-scoped and
   creates the resource group, so assign **Contributor** at subscription scope:

   ```sh
   subId=$(az account show --query id -o tsv)
   az role assignment create --assignee "$appId" \
     --role Contributor --scope "/subscriptions/$subId"
   ```

4. **Configure repository secrets and variables** (Settings → Secrets and
   variables → Actions).

   Secrets:

   | Secret | Description |
   | ------ | ----------- |
   | `AZURE_CLIENT_ID`         | `appId` of the app registration above. |
   | `AZURE_TENANT_ID`         | Directory (tenant) ID. |
   | `AZURE_SUBSCRIPTION_ID`   | Target subscription ID. |
   | `POSTGRES_ADMIN_PASSWORD` | Must match the provisioned Postgres server's password (Bicep re-applies it on every deploy). |
   | `GHCR_PULL_TOKEN`         | PAT with `read:packages` so Azure Container Apps can pull the images. Only needed if the GHCR packages are **private** — see below. |

   Variables:

   | Variable               | Description |
   | ---------------------- | ----------- |
   | `AZURE_ENV_NAME`       | Environment name used for resource naming (e.g. `wasm-registry`). Must match what you provisioned. |
   | `AZURE_LOCATION`       | Azure region (e.g. `centralus`). |
   | `AZURE_RESOURCE_GROUP` | Optional. Overrides the default `rg-<AZURE_ENV_NAME>`. |
   | `CUSTOM_DOMAIN_NAME`   | Optional. Apex domain for the frontend (see section 7). |

   The [`scripts/setup-azure-deploy.sh`](../scripts/setup-azure-deploy.sh)
   helper sets all of these with `gh`. When `azd` and an environment are
   available it **auto-detects the values you already provisioned** —
   `AZURE_ENV_NAME`, `AZURE_LOCATION`, `AZURE_SUBSCRIPTION_ID`,
   `AZURE_TENANT_ID`, `AZURE_RESOURCE_GROUP`, `CUSTOM_DOMAIN_NAME`, and
   `POSTGRES_ADMIN_PASSWORD` — reading them from the azd environment store with
   `azd env get-value` and prompting only for what's left (`AZURE_CLIENT_ID`
   and, if needed, `GHCR_PULL_TOKEN`, which azd does not track). Explicit
   environment variables always win over azd. It **skips any secret or variable
   that is already set on the repo**, so it's safe to re-run after adding a
   single new value; pass `-f` to overwrite existing ones:

   ```sh
   ./scripts/setup-azure-deploy.sh                  # auto-detect from the default azd env
   ./scripts/setup-azure-deploy.sh -e wasm-registry # read from a named azd environment
   ./scripts/setup-azure-deploy.sh -n               # skip azd; prompt for everything
   ./scripts/setup-azure-deploy.sh -a               # backfill tenant + subscription from `az`
   ./scripts/setup-azure-deploy.sh -f               # overwrite values already set on the repo
   ```

   Or set them by hand:

   ```sh
   gh secret set AZURE_CLIENT_ID          # prompts for the value
   gh secret set AZURE_TENANT_ID
   gh secret set AZURE_SUBSCRIPTION_ID
   gh secret set POSTGRES_ADMIN_PASSWORD
   gh secret set GHCR_PULL_TOKEN          # only if the images are private
   gh variable set AZURE_ENV_NAME --body wasm-registry
   gh variable set AZURE_LOCATION --body centralus
   # optional:
   gh variable set AZURE_RESOURCE_GROUP --body rg-wasm-registry
   gh variable set CUSTOM_DOMAIN_NAME --body wasm.directory
   ```

### Container image visibility

If `GHCR_PULL_TOKEN` is set, the `deploy` job wires GHCR pull credentials
(`REGISTRY_SERVER=ghcr.io`, `REGISTRY_USERNAME` = the release actor,
`REGISTRY_PASSWORD=GHCR_PULL_TOKEN`) into the Container Apps, so the token must
belong to an account that can read the packages.

If the `backend` and `frontend` packages are **public** (GHCR package → Package
settings → Change visibility), leave `GHCR_PULL_TOKEN` **unset**: the job then
configures no registry credentials and Azure pulls the images anonymously. Do
not set it to a placeholder value — the Bicep modules treat any non-empty
`registryServer` as a private registry (`useRegistry = !empty(registryServer)`),
so a bogus token would configure a registry with an invalid password and break
image pulls.

### Running and observing a deploy

Trigger a release as usual (`just release`, or the **Release** workflow's *Run
workflow* button). The `deploy` job runs after the images are published; watch
its progress under **Actions**, and the deployment lifecycle (`in_progress` →
`success`/`failure`, with the live frontend URL) under
**Environments → production**.

[deployments]: https://docs.github.com/en/rest/deployments/deployments

## Troubleshooting

- **Prompted for `environmentName` or `postgresAdminPassword` despite values
  being set.** azd loads parameters from `infra/main.bicepparam` (a file
  whose name matches the `infra.module` in [azure.yaml](../azure.yaml)). If
  the file is renamed or missing, azd falls back to prompting for any
  parameter without a default.
- **`LocationIsOfferRestricted` for Postgres.** Your subscription cannot
  provision Postgres Flexible Server in the chosen region. Pick another
  region (see the probe script above).
- **`Microsoft.App` stuck in `Registering` for >30 min.** Try cycling the
  registration: `az provider unregister --namespace Microsoft.App` followed
  by `az provider register --namespace Microsoft.App --consent-to-permissions`.
  If it remains stuck, open an Azure support ticket under
  "Subscription management → Resource provider registration".
- **Wrong API version.** The `NoRegisteredProviderFound` error includes
  the list of supported API versions for the resource type and region. Pin
  Bicep resources to one of the listed GA versions.
- **Managed certificate stuck in `Pending` (custom domain).** There are two
  usual causes. First, the certificate was requested with
  `--validation-method TXT`. Apex domains must use `HTTP`, and TXT validation
  proved unreliable for the `api` subdomain too (the cert stayed stuck in
  `Pending`), so bind **both** hostnames with `HTTP`. Delete the pending cert
  and re-bind with `HTTP`:

  ```sh
  RG="$(azd env get-value AZURE_RESOURCE_GROUP)"
  ENVNAME="$(azd env get-value AZURE_CONTAINER_APPS_ENVIRONMENT_NAME)"
  az containerapp env certificate list -g "$RG" -n "$ENVNAME" \
    --query "[].{name:name, state:properties.provisioningState}" -o table
  az containerapp env certificate delete -g "$RG" -n "$ENVNAME" \
    --certificate <pending-cert-name> --yes
  ```

  Second, the app redirects HTTP→HTTPS (`allowInsecure: false`), so DigiCert's
  HTTP validation probe never reaches it. Bind with `--allow-insecure true` set
  on the ingress, then restore it — see step 2 of
  [§7](#7-optional-bind-a-custom-domain). Note that the per-certificate
  `validationToken` shown for a `TXT`-validated cert is a dead end for these
  hostnames; don't chase it.
