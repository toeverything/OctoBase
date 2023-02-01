job "affine-cloud-stage" {
  region      = "global"
  datacenters = ["production"]
  namespace   = "production"

  type = "service"

  update {
    stagger      = "30s"
    max_parallel = 2
  }

  # Defines a series of tasks that should be co-located on the same Nomad client.
  group "affine-cloud-stage" {
    count = 1

    network {
      port "affine-cloud" {
        to           = 3000
        host_network = "tailscale"
      }
      port "postgres" {
        to           = 5432
        host_network = "tailscale"
      }
      //   port "apiproxy" {
      //     static       = 11002
      //     to           = 3001
      //     host_network = "tailscale"
      //   }
    }

    service {
      tags = ["urlprefix-stage.affine.live/"]
      port = "affine-cloud"
      check {
        name     = "Affine Cloud Stage Check"
        type     = "http"
        path     = "/api/healthz"
        interval = "10s"
        timeout  = "2s"
      }
    }

    task "affine-cloud" {
      driver = "docker"

      env {
        DATABASE_URL        = "postgresql://affine:affine@${NOMAD_ADDR_postgres}/affine"
        SIGN_KEY            = ""
        MAIL_ACCOUNT        = ""
        MAIL_PASSWORD       = ""
        MAIL_FROM           = "noreply@toeverything.info"
        MAIL_PROVIDER       = "smtp.gmail.com"
        MAIL_INVITE_TITLE   = "{{inviter_name}} invited you to join {{workspace_name}}"
        MAIL_INVITE_FILE    = "/app/invite.html"
        SITE_URL            = "https://stage.affine.live"
        DOC_STORAGE_PATH    = "/docs"
        BLOB_STORAGE_PATH   = "/blobs"
        FIREBASE_PROJECT_ID = "pathfinder-52392"
        # GOOGLE_ENDPOINT = "http://100.77.180.48:11002"
        # GOOGLE_ENDPOINT_PASSWORD = "Dct4pq9E9V"
      }
      config {
        image      = "ghcr.io/toeverything/cloud:canary-a885e6f8a0c65e064c319991c1ad8ff37ba0d403"
        force_pull = true
        ports      = ["affine-cloud"]
        volumes = [
          "/home/affineos2022/affine-cloud-stage/docs:/docs",
          "/home/affineos2022/affine-cloud-stage/blobs:/blobs"
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
        hook    = "prestart"
        sidecar = false
      }
      env {
        PGARGS = "-h ${NOMAD_IP_postgres} -p ${NOMAD_HOST_PORT_postgres} -U affine"
      }
      config {
        command = "sh"
        args    = ["-c", "while ! pg_isready ${PGARGS}; do echo \"Waiting for database ${NOMAD_ADDR_postgres}\"; sleep 2; done"]
      }
    }

    task "postgres" {
      driver = "docker"

      lifecycle {
        hook    = "prestart"
        sidecar = true
      }
      env {
        POSTGRES_USER     = "affine"
        POSTGRES_PASSWORD = "affine"
        POSTGRES_DB       = "affine"
      }
      config {
        image      = "postgres"
        force_pull = true
        ports      = ["postgres"]
        volumes    = ["/home/affine/affine-cloud-stage/database:/var/lib/postgresql/data"]
      }
      resources {
        cpu    = 100 # MHz
        memory = 64  # MB
      }
    }
  }
}