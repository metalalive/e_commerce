import socket
import time
import unittest
from unittest.mock import patch, MagicMock

from ecommerce_common.logging.logger import ExtendedLogger
from ecommerce_common.util.messaging.rpc import (
    RPCproxy,
    RpcReplyEvent,
    KombuQueue,
    KombuOperationalError,
)
from ecommerce_common.util.messaging.amqp import (
    AMQPQueueConsumer,
    KombuProducer,
    KombuConsumerMixin,
    UndeliverableMessage,
)


class RpcProxyTestCase(unittest.TestCase):
    DEFAULT_CHANNEL_LABEL = "/utest-channel"

    def setUp(self):
        self.mocked_channel = MagicMock()
        self.mocked_channel.channel_id = self.DEFAULT_CHANNEL_LABEL

    def tearDown(self):
        del self.mocked_channel

    # Note that the patch works ONLY in the function it is imported to,
    # NOT the function where it is originally declared.
    # https://stackoverflow.com/questions/8575713/
    @patch("common.util.python.messaging.rpc.get_connection")
    @patch.object(KombuConsumerMixin, attribute="consume")
    @patch.object(KombuProducer, attribute="publish")
    @patch.object(KombuQueue, attribute="delete")
    @patch.object(KombuQueue, attribute="declare")
    @patch.object(AMQPQueueConsumer, attribute="_init_default_conn")
    def test_pubsub_ok(self, def_conn, q_create, q_rm, prod_pub, mo_consume, ctx_mgr):
        q_create.return_value = "my-queue-1 created"
        prod_pub.return_value = "msg published"
        q_rm.return_value = "my-queue-1 removed"
        def_conn.return_value.default_channel = self.mocked_channel
        ctx_mgr.return_value.__enter__.return_value.default_channel = (
            self.mocked_channel
        )
        rpc1 = RPCproxy(dst_app_name="remote-site-1", src_app_name="local-app")
        # --- start publishing message
        evt = rpc1.drill_holes(num=2, deep_mm=65)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.INITED)
        self.assertFalse(evt.result["timeout"])
        q_create.assert_called_once()
        prod_pub.assert_called_once()
        corr_id = evt.result["corr_id"]
        # --- refresh event, remote site should reply with some progress
        mock_reply_metadata = MagicMock()
        mock_reply_metadata.properties = {"correlation_id": corr_id}

        def mock_remote_reply(*args, **kwargs):
            mock_body = {"status": RpcReplyEvent.status_opt.STARTED, "result": 9204071}
            rpc1._rpc_reply_listener.handle_message(
                message=mock_reply_metadata, body=mock_body
            )
            yield

        mo_consume.side_effect = mock_remote_reply
        actual_err = evt.refresh(retry=False, timeout=0.1, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.STARTED)
        self.assertEqual(evt.result["result"], 9204071)
        self.assertIsNone(actual_err)
        self.assertEqual(mo_consume.call_count, 1)

        def mock_remote_reply(limit, timeout):
            mock_body = {"status": RpcReplyEvent.status_opt.SUCCESS, "result": 400830}
            rpc1._rpc_reply_listener.handle_message(
                message=mock_reply_metadata, body=mock_body
            )
            yield

        mo_consume.side_effect = mock_remote_reply
        actual_err = evt.refresh(retry=False, timeout=0.03, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.SUCCESS)
        self.assertEqual(evt.result["result"], 400830)
        self.assertIsNone(actual_err)
        self.assertEqual(mo_consume.call_count, 2)
        ### def_conn.assert_called_once()
        q_rm.assert_not_called()
        del rpc1

    ## end of def test_pubsub_ok

    @patch("common.util.python.messaging.rpc.get_connection")
    @patch.object(KombuProducer, attribute="publish")
    @patch.object(KombuQueue, attribute="delete")
    @patch.object(KombuQueue, attribute="declare")
    @patch.object(AMQPQueueConsumer, attribute="_init_default_conn")
    def test_pub_timeout(self, def_conn, q_create, q_rm, prod_pub, ctx_mgr):
        q_create.return_value = "my-queue-1 created"
        q_rm.return_value = "my-queue-1 removed"
        def_conn.return_value.default_channel = self.mocked_channel
        ctx_mgr.return_value.__enter__.side_effect = KombuOperationalError(
            "Errno 111 Unit Test Connection Error"
        )
        rpc1 = RPCproxy(dst_app_name="remote-site-1", src_app_name="local-app")
        # --- start publishing message
        evt = rpc1.drill_holes(num=2, deep_mm=65)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.FAIL_CONN)
        q_create.assert_not_called()
        prod_pub.assert_not_called()
        q_rm.assert_not_called()
        del rpc1

    @patch("common.util.python.messaging.rpc.get_connection")
    @patch.object(KombuProducer, attribute="publish")
    @patch.object(KombuQueue, attribute="delete")
    @patch.object(KombuQueue, attribute="declare")
    @patch.object(AMQPQueueConsumer, attribute="_init_default_conn")
    def test_pub_fail(self, def_conn, q_create, q_rm, prod_pub, ctx_mgr):
        q_create.return_value = "my-queue-1 created"
        q_rm.return_value = "my-queue-1 removed"
        def_conn.return_value.default_channel = self.mocked_channel
        ctx_mgr.return_value.__enter__.return_value.default_channel = (
            self.mocked_channel
        )
        prod_pub.side_effect = UndeliverableMessage(
            exchange="ut-exchange", routing_key="utest.app.method"
        )
        rpc1 = RPCproxy(dst_app_name="remote-site-1", src_app_name="local-app")
        # --- start publishing message
        evt = rpc1.drill_holes(num=2, deep_mm=65)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.FAIL_PUBLISH)
        q_create.assert_called_once()
        prod_pub.assert_called_once()
        q_rm.assert_not_called()
        del rpc1

    @patch("common.util.python.messaging.rpc.get_connection")
    @patch.object(KombuConsumerMixin, attribute="consume")
    @patch.object(KombuProducer, attribute="publish")
    @patch.object(KombuQueue, attribute="delete")
    @patch.object(KombuQueue, attribute="declare")
    @patch.object(AMQPQueueConsumer, attribute="_init_default_conn")
    def test_sub_timeout(self, def_conn, q_create, q_rm, prod_pub, mo_consume, ctx_mgr):
        q_create.return_value = "my-queue-1 created"
        prod_pub.return_value = "msg published"
        q_rm.return_value = "my-queue-1 removed"
        def_conn.return_value.default_channel = self.mocked_channel
        ctx_mgr.return_value.__enter__.return_value.default_channel = (
            self.mocked_channel
        )
        reply_wait_max_secs = 1
        rpc1 = RPCproxy(
            dst_app_name="remote-site-1",
            src_app_name="local-app",
            reply_timeout_sec=reply_wait_max_secs,
        )
        # --- start publishing message
        evt = rpc1.drill_holes(num=2, deep_mm=65)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.INITED)
        self.assertFalse(evt.result["timeout"])
        q_create.assert_called_once()
        prod_pub.assert_called_once()
        corr_id = evt.result["corr_id"]
        # --- refresh event, remote site never responds
        mo_consume.side_effect = socket.timeout("unit test mock")
        self.assertEqual(mo_consume.call_count, 0)
        actual_err = evt.refresh(retry=False, timeout=0.2, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.INITED)
        self.assertFalse(evt.result["timeout"])
        self.assertTrue(isinstance(actual_err, socket.timeout))
        self.assertEqual(mo_consume.call_count, 1)
        time.sleep(reply_wait_max_secs + 0.1)
        actual_err = evt.refresh(retry=False, timeout=0.2, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.INITED)
        self.assertTrue(evt.result["timeout"])
        self.assertIsNone(actual_err)  # no error reported after user-defined timeout
        self.assertEqual(mo_consume.call_count, 1)
        actual_err = evt.refresh(retry=True, timeout=0.2, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.INITED)
        self.assertFalse(evt.result["timeout"])
        self.assertTrue(isinstance(actual_err, socket.timeout))
        self.assertEqual(mo_consume.call_count, 2)
        q_rm.assert_not_called()
        del rpc1

    @patch("common.util.python.messaging.rpc.get_connection")
    @patch.object(KombuConsumerMixin, attribute="consume")
    @patch.object(KombuProducer, attribute="publish")
    @patch.object(KombuQueue, attribute="delete")
    @patch.object(KombuQueue, attribute="declare")
    @patch.object(AMQPQueueConsumer, attribute="_init_default_conn")
    def test_sub_reply_error(
        self, def_conn, q_create, q_rm, prod_pub, mo_consume, ctx_mgr
    ):
        q_create.return_value = "my-queue-1 created"
        prod_pub.return_value = "msg published"
        q_rm.return_value = "my-queue-1 removed"
        def_conn.return_value.default_channel = self.mocked_channel
        ctx_mgr.return_value.__enter__.return_value.default_channel = (
            self.mocked_channel
        )
        rpc1 = RPCproxy(dst_app_name="remote-site-1", src_app_name="local-app")
        # --- start publishing message
        evt = rpc1.drill_holes(num=2, deep_mm=65)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.INITED)
        self.assertFalse(evt.result["timeout"])
        q_create.assert_called_once()
        prod_pub.assert_called_once()
        corr_id = evt.result["corr_id"]
        # --- refresh event, remote site should reply with some progress
        mock_reply_metadata = MagicMock()
        mock_reply_metadata.properties = {"correlation_id": corr_id}

        def mock_remote_reply(*args, **kwargs):
            mock_body = {
                "status": RpcReplyEvent.status_opt.STARTED,
                "result": "consumer received",
            }
            rpc1._rpc_reply_listener.handle_message(
                message=mock_reply_metadata, body=mock_body
            )
            yield

        mo_consume.side_effect = mock_remote_reply
        actual_err = evt.refresh(retry=False, timeout=0.1, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.STARTED)
        self.assertEqual(evt.result["result"], "consumer received")
        self.assertIsNone(actual_err)

        def mock_remote_reply(limit, timeout):
            mock_body = {
                "status": RpcReplyEvent.status_opt.REMOTE_ERROR,
                "result": "process failure",
            }
            rpc1._rpc_reply_listener.handle_message(
                message=mock_reply_metadata, body=mock_body
            )
            yield

        mo_consume.side_effect = mock_remote_reply
        evt.refresh(retry=False, timeout=0.03, num_of_msgs_fetch=1)
        self.assertEqual(evt.result["status"], RpcReplyEvent.status_opt.REMOTE_ERROR)
        self.assertEqual(evt.result["result"], "process failure")
        q_rm.assert_not_called()
        del rpc1


## end of class RpcProxyTestCase
