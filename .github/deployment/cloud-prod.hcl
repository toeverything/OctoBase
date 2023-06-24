job "affine-cloud-prod" {
  region      = "global"
  datacenters = ["production"]
  namespace   = "production"

  type = "service"

  update {
    stagger      = "30s"
    max_parallel = 2
  }

  # Defines a series of tasks that should be co-located on the same Nomad client.
  group "affine-cloud-prod" {
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
    }

    service {
      tags = ["urlprefix-api.affine.pro/api/"]
      port = "affine-cloud"
      check {
        name     = "Affine Cloud Production Check"
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
DOCKER_TAG    = "{{ key "service/production/affine-cloud/tag" }}"
DATABASE_URL  = "postgresql://affine:{{ key "service/production/affine-cloud/database_password" }}@{{ env "NOMAD_ADDR_postgres" }}/affine"
SIGN_KEY      = "{{ key "service/production/affine-cloud/sign_key" }}"
MAIL_ACCOUNT  = "{{ key "service/production/affine-cloud/mail_account" }}"
MAIL_PASSWORD = "{{ key "service/production/affine-cloud/mail_password" }}"
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
        cpu    = 1024 # MHz
        memory = 1024 # MB
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
DATABASE_INSTANCE = "{{ key "service/production/affine-cloud/database_instance" }}"
EOH

        destination = "secrets/.env"
        env         = true
      }

      template {
        change_mode = "noop"
        destination = "local/service-account-key.json"
        data        = <<EOH
{{ key "service/production/affine-cloud/database_token" }}
EOH
      }

      config {
        image      = "gcr.io/cloud-sql-connectors/cloud-sql-proxy:2.0.0"
        force_pull = true
        ports      = ["postgres"]
        args       = ["${DATABASE_INSTANCE}", "-c=/service-account-key.json", "--address=0.0.0.0", "--private-ip"]
        volumes    = ["local/service-account-key.json:/service-account-key.json"]
      }
      resources {
        cpu    = 128 # MHz
        memory = 128 # MB
      }
    }
  }
}