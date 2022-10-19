job "jwst" {
  region      = "global"
  datacenters = ["scholar"]

  type = "service"

  update {
    stagger      = "30s"
    max_parallel = 2
  }

  # Defines a series of tasks that should be co-located on the same Nomad client.
  group "mysc" {
    count = 1

    network {
      port "mysc" {
        # Specifies the static TCP/UDP port to allocate.
        static       = 11000
        to           = 3000
        host_network = "tailscale"
      }
    }

    service {
      check {
        name     = "Jwst Mysc Check"
        port     = "mysc"
        type     = "http"
        path     = "/"
        interval = "10s"
        timeout  = "2s"
      }
    }

    task "mysc" {
      driver = "docker"

      config {
        image       = "ghcr.io/toeverything/mysc:canary-5da7c7e739363e809ffb94c3d3023ddaf5658c35"
        force_pull  = true
        ports       = ["mysc"]
        labels      = {
          "io.portainer.accesscontrol.teams" = "development"
        }
      }
      resources {
        cpu    = 100 # MHz
        memory = 64  # MB
      }
    }
  }
}