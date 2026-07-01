# Deploying to Azure

This guide walks through provisioning the registry's infrastructure on Azure
using the Azure Developer CLI (`azd`). The deployment uses
[infra/main.bicep](../infra/main.bicep) to create a resource group, Log
Analytics workspace, Container Apps environment, two Container Apps
(frontend + backend), and an Azure Database for PostgreSQL Flexible Server.

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

2. **Bind the hostname and request a managed certificate.** Once the records
   resolve publicly, run:

   ```sh
   az containerapp hostname bind \
     --resource-group "$(azd env get-value AZURE_RESOURCE_GROUP)" \
     --name "$(azd env get-value SERVICE_FRONTEND_NAME)" \
     --hostname "$(azd env get-value CUSTOM_DOMAIN_NAME)" \
     --environment "$(azd env get-value AZURE_CONTAINER_APPS_ENVIRONMENT_NAME)" \
     --validation-method TXT
   ```

   This adds the hostname, validates ownership via the `asuid` `TXT` record,
   and provisions a free Azure-managed TLS certificate that auto-renews.
   Certificate issuance can take a few minutes. The command is idempotent, so
   it is safe to re-run if DNS had not finished propagating the first time.

After it completes, the site is reachable at `https://wasm.directory`.

## 8. Tear down

To delete everything provisioned by this template:

```sh
azd down --purge --force
```

`--purge` also removes soft-deleted resources (Key Vault, Log Analytics)
so the same `AZURE_ENV_NAME` can be reused immediately.

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
