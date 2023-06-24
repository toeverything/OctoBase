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

    restart {
      attempts = 3
      delay    = "10s"
      interval = "1m"
      mode     = "fail"
    }

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
      tags = ["urlprefix-api-stage.affine.live/"]
      port = "affine-cloud"
      check {
        name     = "Affine Cloud Stage Check"
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
        AFFINE_CLOUD_LOG             = "info,mio=off,hyper=off,rustls=off,tantivy=off,sqlx::query=off,jwst_rpc=trace,jwst_rpc::context=info,affine_cloud=trace"
        JWT_ACCESS_TOKEN_EXPIRES_IN  = "3600"
        JWT_REFRESH_TOKEN_EXPIRES_IN = "2592000"
      }
      template {
        data = <<EOH
DOCKER_TAG    = "{{ key "service/stage/affine-cloud/tag" }}"
DATABASE_URL  = "postgresql://affine:{{ key "service/stage/affine-cloud/database_password" }}@{{ env "NOMAD_ADDR_postgres" }}/affine"
SIGN_KEY      = "{{ key "service/stage/affine-cloud/sign_key" }}"
MAIL_ACCOUNT  = "{{ key "service/stage/affine-cloud/mail_account" }}"
MAIL_PASSWORD = "{{ key "service/stage/affine-cloud/mail_password" }}"
EOH

        destination = "secrets/.env"
        env         = true
      }

      config {
        image      = "ghcr.io/toeverything/cloud-self-hosted:${DOCKER_TAG}"
        force_pull = true
        ports      = ["affine-cloud"]
      }
      resources {
        cpu    = 400 # MHz
        memory = 256 # MB
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
POSTGRES_PASSWORD = "{{ key "service/stage/affine-cloud/database_password" }}"
EOH

        destination = "secrets/.env"
        env         = true
      }

      template {
        change_mode = "noop"
        destination = "local/init.sql"
        data        = <<EOH
CREATE DATABASE affine_binary;
GRANT ALL PRIVILEGES ON DATABASE affine_binary TO affine;
EOH
      }

      config {
        image      = "postgres"
        force_pull = true
        ports      = ["postgres"]
        volumes = [
          "/home/affineos2022/affine-cloud-stage/database:/var/lib/postgresql/data",
          "local/init.sql:/docker-entrypoint-initdb.d/init.sql"
        ]

      }
      resources {
        cpu    = 400 # MHz
        memory = 256 # MB
      }
    }
  }
}