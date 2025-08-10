## Pre-requisite
| type | name | versions support |
|------|------|------------------|
| Message Queue | RabbitMQ | `3.13` |


## Build
### RabbitMQ server
run RabbitMQ server as AMQP message broker for inter-application communication 

```bash
cd /path/to/ecomm-proj/infra
docker build --tag rabbitmq-custom-init:3.13-management  --file ./rabbitmq-custom-init.dockerfile  .
```

### Cron Job Scheduler
```bash
cd /path/to/ecomm-proj/
docker build --tag ecom-common-py:latest --file ./infra/commonpython.dockerfile .
```

### Centralized logging server, dashboard for monitoring system status
TODO


## Run Services
```bash
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
