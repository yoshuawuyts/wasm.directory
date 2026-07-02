# Deploying to Azure

This guide walks through provisioning the registry's infrastructure on Azure
using the Azure Developer CLI (`azd`). The deployment uses
[infra/main.bicep](../infra/main.bicep) to create a resource group, Log
Analytics workspace, Container Apps environment, two Container Apps
(frontend + backend), and an Azure Database for PostgreSQL Flexible Server.

> **Two ways to deploy.** The numbered steps below are the manual `azd`
> walkthrough. To deploy automatically from CI instead — including GitHub
> deployment status tracking — see
> [Automated deployment via GitHub Actions](#automated-deployment-via-github-actions).

## Prerequisites

Install the following tools:

- [Azure Developer CLI (`azd`)](https://learn.microsoft.com/azure/developer/azure-developer-cli/install-azd) — v1.25 or newer
- [Azure CLI (`az`)](https://learn.microsoft.com/cli/azure/install-azure-cli)
- [Docker](https://docs.docker.com/get-docker/) — required to build the
  `frontend` and `backend` service images during `azd deploy`

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
| `CUSTOM_DOMAIN_NAME`      | no       | Apex domain to serve the frontend on (e.g. `wasm.directory`). When set, provisioning also creates a DNS zone for it. See [Bind a custom domain](#7-optional-bind-a-custom-domain). |

Set them with `azd env set`:

```sh
azd env set AZURE_SUBSCRIPTION_ID '<your-subscription-id>'
azd env set AZURE_LOCATION centralus
azd env set POSTGRES_ADMIN_PASSWORD '<a-strong-password>'
azd env set BACKEND_IMAGE 'ghcr.io/<owner>/component-cli/backend:latest'
azd env set FRONTEND_IMAGE 'ghcr.io/<owner>/component-cli/frontend:latest'
```

Confirm what's stored:

```sh
azd env get-values
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
  ingress IP, and
- an `asuid` `TXT` record carrying the frontend's domain-verification id, used
  by Azure to validate ownership before issuing a managed certificate.

The bind itself is a one-time manual step because it depends on your registrar
delegating the zone to Azure — something only you can do:

1. **Delegate the zone.** Read the name servers Azure assigned and point your
   registrar's `NS` records at exactly that set:

   ```sh
   azd env get-value DNS_NAME_SERVERS
   ```

   Then wait for propagation (`dig +short NS wasm.directory` should return the
   Azure name servers; `dig +short TXT asuid.wasm.directory` should return the
   verification id).

2. **Add the hostname, then bind a managed certificate.** Once the records
   resolve publicly, register the hostname and issue the certificate. Capture
   the names first to keep the commands short:

   ```sh
   RG="$(azd env get-value AZURE_RESOURCE_GROUP)"
   APP="$(azd env get-value SERVICE_FRONTEND_NAME)"
   ENVNAME="$(azd env get-value AZURE_CONTAINER_APPS_ENVIRONMENT_NAME)"
   DOMAIN="$(azd env get-value CUSTOM_DOMAIN_NAME)"
   ```

   First add the custom hostname. Azure validates ownership against the
   `asuid` `TXT` record from step 1. (A one-shot `bind` without adding the
   hostname first fails with `RequireCustomHostnameInEnvironment`.)

   ```sh
   az containerapp hostname add \
     --resource-group "$RG" --name "$APP" --hostname "$DOMAIN"
   ```

   > **Apex domains validate over `HTTP`, not `TXT`.** Azure issues managed
   > certificates for an apex domain (like `wasm.directory`) by reaching the
   > app over HTTP from DigiCert's IP addresses — `--validation-method TXT` is
   > only for subdomains and leaves an apex certificate stuck in `Pending`.
   > See the
   > [managed-certificate requirements](https://learn.microsoft.com/azure/container-apps/custom-domains-managed-certificates#free-certificate-requirements).

   The frontend redirects HTTP→HTTPS (`allowInsecure: false`), which blocks
   DigiCert's HTTP validation probe and would leave the certificate `Pending`
   indefinitely. Temporarily allow insecure traffic, bind the certificate
   (which issues the managed cert and enables SNI), then restore the redirect:

   ```sh
   # 1. Let DigiCert reach the app over plain HTTP for validation
   az containerapp ingress update -n "$APP" -g "$RG" --allow-insecure true

   # 2. Issue + bind the free managed TLS certificate (HTTP validation for apex)
   az containerapp hostname bind \
     --resource-group "$RG" --name "$APP" --hostname "$DOMAIN" \
     --environment "$ENVNAME" --validation-method HTTP

   # 3. Restore the HTTP→HTTPS redirect
   az containerapp ingress update -n "$APP" -g "$RG" --allow-insecure false
   ```

   With the redirect out of the way, issuance usually completes within a
   minute or two (Azure allows up to 20). The certificate auto-renews
   afterwards. Verify the site serves a valid certificate:

   ```sh
   curl -sS -o /dev/null -w '%{http_code}\n' "https://$DOMAIN/"   # expect 200
   ```

After it completes, the site is reachable at `https://wasm.directory`.

## 8. Tear down

To delete everything provisioned by this template:

```sh
azd down --purge --force
```

`--purge` also removes soft-deleted resources (Key Vault, Log Analytics)
so the same `AZURE_ENV_NAME` can be reused immediately.

## Automated deployment via GitHub Actions

The [`Release` workflow](../.github/workflows/release.yml) deploys to Azure
automatically. After its `publish-images` job builds and pushes the `backend`
and `frontend` images to GHCR, the `deploy` job rolls them out to the Container
Apps and records the rollout through the [GitHub Deployments API][deployments]
(visible under the repository's **Environments → production** tab).

The `deploy` job performs a full, idempotent Bicep redeploy —
`az deployment sub create` against [infra/main.bicep](../infra/main.bicep) with
parameters read from [infra/main.bicepparam](../infra/main.bicepparam), the same
`readEnvironmentVariable(...)` wiring `azd provision` uses — so the Bicep
template stays the single source of truth and secrets never touch the command
line. Images are pinned to the exact released `:X.Y.Z` tag (not `:latest`), and
the frontend URL is reported back as the deployment's `environment_url`.

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
   helper sets all of these with `gh`, prompting for anything not already in
   the environment (secret values are read without echo). It **skips any
   secret or variable that is already set on the repo**, so it's safe to re-run
   after adding a single new value; pass `-f` to overwrite existing ones:

   ```sh
   ./scripts/setup-azure-deploy.sh -a   # -a fills tenant + subscription from `az`
   ./scripts/setup-azure-deploy.sh -f   # overwrite values already set on the repo
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

The `deploy` job wires GHCR pull credentials (`REGISTRY_SERVER=ghcr.io`,
`REGISTRY_USERNAME` = the release actor, `REGISTRY_PASSWORD=GHCR_PULL_TOKEN`)
into the Container Apps, so `GHCR_PULL_TOKEN` must belong to an account that can
read the packages. Alternatively, make the `backend` and `frontend` packages
**public** (GHCR package → Package settings → Change visibility); Azure then
pulls them anonymously and `GHCR_PULL_TOKEN` can be any placeholder value.

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
- **Managed certificate stuck in `Pending` (custom domain).** For an apex
  domain there are two usual causes. First, the certificate was requested with
  `--validation-method TXT`; apex domains must use `HTTP`. Delete the pending
  cert and re-bind with `HTTP`:

  ```sh
  RG="$(azd env get-value AZURE_RESOURCE_GROUP)"
  ENVNAME="$(azd env get-value AZURE_CONTAINER_APPS_ENVIRONMENT_NAME)"
  az containerapp env certificate list -g "$RG" -n "$ENVNAME" \
    --query "[].{name:name, state:properties.provisioningState}" -o table
  az containerapp env certificate delete -g "$RG" -n "$ENVNAME" \
    --certificate <pending-cert-name> --yes
  ```

  Second, the frontend redirects HTTP→HTTPS (`allowInsecure: false`), so
  DigiCert's HTTP validation probe never reaches the app. Bind with
  `--allow-insecure true` set on the ingress, then restore it — see step 2 of
  [§7](#7-optional-bind-a-custom-domain). Note that the per-certificate
  `validationToken` shown for a `TXT`-validated cert is a dead end for apex
  domains; don't chase it.
