## Pre-requisite
| type | name | versions support |
|------|------|------------------|
| Message Queue | RabbitMQ | from `3.2.4` to `3.13` |


## Run essential services
#### AMQP message broker for inter-application communication

```bash
service rabbitmq-server start
service rabbitmq-server stop
```

#### Cron Job Consumer and scheduler
See [`README.md`](./services/common/python/README.md) in common python project setup

#### Centralized logging server, dashboard for monitoring system status
TODO

