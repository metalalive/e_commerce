## Pre-requisite
| type | name | versions support |
|------|------|------------------|
| Message Queue | RabbitMQ | `3.13` |


## Build and Run Services
The commands below will build and run all inter-application servers at once

- AMQP message broker for inter-application communication

```bash
cd /path/to/ecomm-proj/infra

docker build --tag rabbitmq-custom-init:3.13-management  --file ./rabbitmq-custom-init.dockerfile  .

docker compose --file ./docker-compose-generic.yml --file  ./docker-compose-dev.yml up --detach
```

To stop all servers without removing the containers, use the command below:
```bash
docker compose --file ./docker-compose-generic.yml --file  ./docker-compose-dev.yml stop
```

Or stop and then remove all relevant containers:
```bash
docker compose --file ./docker-compose-generic.yml --file  ./docker-compose-dev.yml down
```


#### Cron Job Consumer and scheduler
See [`README.md`](./services/common/python/README.md) in common python project setup

#### Centralized logging server, dashboard for monitoring system status
TODO

