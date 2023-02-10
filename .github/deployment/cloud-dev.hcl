job "affine-cloud-dev" {
  region      = "global"
  datacenters = ["development"]
  namespace   = "development"

  type = "service"

  update {
    stagger      = "30s"
    max_parallel = 2
  }

  # Defines a series of tasks that should be co-located on the same Nomad client.
  group "affine-cloud-dev" {
    count = 1

    restart {
      attempts = 3
      delay    = "10s"
      interval = "1m"
      mode     = "fail"
    }

    network {
      port "affine-cloud" {
        static       = 11001
        to           = 3000
        host_network = "tailscale"
      }
      port "postgres" {
        to           = 5432
        host_network = "tailscale"
      }
      port "apiproxy" {
        static       = 11002
        to           = 3001
        host_network = "tailscale"
      }
    }

    service {
      tags = ["urlprefix-dev.affine.live/"]
      port = "affine-cloud"
      check {
        name     = "Affine Cloud Dev Check"
        type     = "http"
        path     = "/api/healthz"
        interval = "10s"
        timeout  = "2s"
        check_restart {
          limit = 3
          grace = "90s"
        }
      }
    }

    task "affine-cloud" {
      driver = "docker"

      env {

      }
      template {
        data = <<EOH
DATABASE_URL        = "postgresql://affine:{{ key "service/development/affine-cloud/database_password" }}@{{ env "NOMAD_ADDR_postgres" }}/affine"
SIGN_KEY            = "{{ key "service/development/affine-cloud/sign_key" }}"
MAIL_ACCOUNT        = "{{ key "service/development/affine-cloud/mail_account" }}"
MAIL_PASSWORD       = "{{ key "service/development/affine-cloud/mail_password" }}"
SITE_URL            = "https://dev.affine.live"
FIREBASE_PROJECT_ID = "pathfinder-52392"
# GOOGLE_ENDPOINT   = "http://100.77.180.48:11002"
# GOOGLE_ENDPOINT_PASSWORD = "Dct4pq9E9V"
EOH

        destination = "secrets/.env"
        env         = true
      }

      config {
        image      = "ghcr.io/toeverything/cloud:canary-1888f2c6271e99b8c318911c436c6a98f7f9faab"
        force_pull = true
        ports      = ["affine-cloud"]
      }
      resources {
        cpu    = 400 # MHz
        memory = 512 # MB
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

      template {
        data = <<EOH
POSTGRES_USER     = "affine"
POSTGRES_PASSWORD = "{{ key "service/development/affine-cloud/database_password" }}"
EOH

        destination = "secrets/.env"
        env         = true
      }

      template {
        change_mode = "noop"
        destination = "local/init.sql"
        data        = <<EOH
CREATE DATABASE affine_blobs;
CREATE DATABASE affine_docs;
GRANT ALL PRIVILEGES ON DATABASE affine_blobs TO affine;
GRANT ALL PRIVILEGES ON DATABASE affine_docs TO affine;
EOH
      }

      config {
        image      = "postgres"
        force_pull = true
        ports      = ["postgres"]
        volumes = [
          "/home/affine/affine-cloud-development:/var/lib/postgresql/data",
          "local/init.sql:/docker-entrypoint-initdb.d/init.sql"
        ]

      }
      resources {
        cpu    = 200 # MHz
        memory = 128 # MB
      }
    }

    task "apiproxy" {
      driver = "docker"

      config {
        image      = "ghcr.io/toeverything/apiproxy:nightly-latest"
        force_pull = true
        ports      = ["apiproxy"]
      }
      resources {
        cpu    = 100 # MHz
        memory = 64  # MB
      }
    }
  }
}