job "affine-cloud" {
    region      = "global"
    datacenters = ["scholar"]

    type = "service"

    update {
        stagger      = "30s"
        max_parallel = 2
    }

    # Defines a series of tasks that should be co-located on the same Nomad client.
    group "affine-cloud" {
        count = 1

        network {
            port "affine-cloud" {
                # Specifies the static TCP/UDP port to allocate.
                static       = 11001
                to           = 3000
                host_network = "tailscale"
            }
            port "postgres" {
                to           = 5432
            }
        }

        service {
            check {
                name     = "Affine Cloud Check"
                port     = "affine-cloud"
                type     = "http"
                path     = "/api/healthz"
                interval = "10s"
                timeout  = "2s"
            }
        }

        task "affine-cloud" {
            driver = "docker"

            env {
                DATABASE_URL = "postgresql://affine:affine@${NOMAD_ADDR_postgres}/affine"
                SIGN_KEY = ""
                MAIL_ACCOUNT = ""
                MAIL_PASSWORD = ""
                MAIL_FROM = "noreply@toeverything.info"
                MAIL_PROVIDER = "smtp.gmail.com"
                MAIL_INVITE_TITLE = "{{inviter_name}} invited you to join {{workspace_name}}"
                MAIL_INVITE_FILE = "/app/invite.html"
                SITE_URL = "https://pathfinder.affine.systems"
                DOC_STORAGE_PATH = "/docs"
                BLOB_STORAGE_PATH = "/blobs"
                FIREBASE_PROJECT_ID = "pathfinder-52392"
            }
            config {
                image       = "ghcr.io/toeverything/cloud:canary-d86de96c8d291f6e0d3365985d52a1b3efa3be02"
                force_pull  = true
                ports       = ["affine-cloud"]
                labels      = {
                    "io.portainer.accesscontrol.teams" = "development"
                }
                volumes     = [
                    "/home/affine/affine-cloud/docs:/docs",
                    "/home/affine/affine-cloud/blobs:/blobs"
                ]
            }
            resources {
                cpu    = 100 # MHz
                memory = 64  # MB
            }
        }

        task "database-init" {
            driver = "exec"

            lifecycle {
                hook        = "prestart"
                sidecar     = false
            }
            env {
                PGARGS      = "-h ${NOMAD_IP_postgres} -p ${NOMAD_HOST_PORT_postgres} -U affine"
            }
            config {
                command     = "sh"
                args        = ["-c", "while ! pg_isready ${PGARGS}; do echo \"Waiting for database ${NOMAD_ADDR_postgres}\"; sleep 2; done"]
            }
        }

        task "postgres" {
            driver = "docker"

            lifecycle {
                hook = "prestart"
                sidecar = true
            }
            env {
                POSTGRES_USER       = "affine"
                POSTGRES_PASSWORD   = "affine"
                POSTGRES_DB         = "affine"
            }
            config {
                image       = "postgres"
                force_pull  = true
                ports       = ["postgres"]
                labels      = {
                    "io.portainer.accesscontrol.teams" = "development"
                }
                volumes     = ["/home/affine/affine-cloud/database:/var/lib/postgresql/data"]
            }
            resources {
                cpu    = 100 # MHz
                memory = 64  # MB
            }
        }
    }
}