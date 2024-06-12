use ecommerce_common::config::{AppAmqpBindingCfg, AppAmqpBindingReplyCfg};

mod amqp;

fn ut_clone_amqp_binding_reply_cfg(src: &AppAmqpBindingReplyCfg) -> AppAmqpBindingReplyCfg {
    AppAmqpBindingReplyCfg {
        queue: src.queue.clone(),
        correlation_id_prefix: src.correlation_id_prefix.clone(),
        ttl_secs: src.ttl_secs,
        max_length: src.max_length,
        durable: src.durable,
    }
}

fn ut_clone_amqp_binding_cfg(src: &AppAmqpBindingCfg) -> AppAmqpBindingCfg {
    AppAmqpBindingCfg {
        queue: src.queue.clone(),
        exchange: src.exchange.clone(),
        routing_key: src.routing_key.clone(),
        ttl_secs: src.ttl_secs,
        max_length: src.max_length,
        durable: src.durable,
        ensure_declare: src.ensure_declare,
        subscribe: src.subscribe,
        reply: src.reply.as_ref().map(ut_clone_amqp_binding_reply_cfg),
        python_celery_task: src.python_celery_task.clone(),
    }
}
