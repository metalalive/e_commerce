import logging, json, os, ssl, smtplib, email
from typing import List, Dict, Optional
from pathlib import Path

from .celery import app as celery_app
from ecommerce_common.logging.util import log_fn_wrapper

_logger = logging.getLogger(__name__)

srv_basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)


def send_email(
    secret: Dict[str, str],
    subject: str,
    body: str,
    sender: str,
    recipients: List[str],
    attachment_paths: List[str],
):
    assert secret.get("password"), "missing password"
    mml = email.message.EmailMessage()
    mml.set_content(body)
    mml["From"] = sender
    mml["To"] = ", ".join(recipients)
    mml["Subject"] = subject
    for att_path in attachment_paths:
        with open(att_path, "rb") as f:
            content = f.read()
            mml.add_attachment(content, maintype="application", subtype="octet-stream")
    # do not use SMTP_SSL
    with smtplib.SMTP(host=secret["host"], port=secret["port"]) as hdlr:
        if secret.get("cert_path"):
            cert_fullpath = Path(secret["cert_path"]).resolve(strict=True)
            assert cert_fullpath.exists(), "cert specified not exists"
            assert cert_fullpath.is_file(), "cert path specified has to be a file"
            sslctx = ssl.SSLContext(protocol=ssl.PROTOCOL_TLS_CLIENT)
            sslctx.verify_mode = ssl.CERT_REQUIRED
            assert sslctx.verify_mode == ssl.CERT_REQUIRED
            sslctx.load_verify_locations(cafile=cert_fullpath)
        else:  # cert should be located in OS default path
            sslctx = None
        hdlr.starttls(context=sslctx)
        hdlr.login(secret["username"], secret["password"])
        hdlr.sendmail(sender, recipients, mml.as_string())
    ## the context manager will close the socket automatically on exit
    ## instead of manually calling `hdlr.sock.close()`


# end of send_email


@celery_app.task(bind=True, queue="mailing", routing_key="mail.defualt")
@log_fn_wrapper(logger=_logger, loglevel=logging.INFO)
def sendmail(
    self,
    to_addrs: List[str],
    from_addr: str,
    subject: str,
    content: str,
    attachment_paths: Optional[List[str]] = None,
):
    # TODO, connection pool for SMTP whenever necessary
    secret = None
    secrets_path = srv_basepath.joinpath("common/data/secrets.json")
    with open(secrets_path, "r") as f:
        d = json.load(f)
        secret = d["backend_apps"]["smtp"]
        cert_relative_path = secret.get("cert_path")
        if cert_relative_path:
            fullpath = srv_basepath.joinpath(cert_relative_path)
            secret["cert_path"] = fullpath
    assert secret, "failed to load SMTP configuration"

    tsk_instance = self
    log_args = [
        "to_addrs",
        to_addrs,
        "from_addr",
        from_addr,
        "subject",
        subject,
        "task_req_id",
        tsk_instance.request.id,
        "task_req_expire",
        tsk_instance.request.expires,
    ]
    _logger.debug(None, *log_args)
    send_email(
        secret=secret,
        subject=subject,
        body=content,
        sender=from_addr,
        recipients=to_addrs,
        attachment_paths=attachment_paths or [],
    )


@celery_app.task
def default_error_handler(task_id):
    result = AsyncResult(task_id)
    exc = result.get(propagate=False)
    errmsg = "task {0} exception : {1!r}\n{2!r}"
    args = [task_id, exc, result.traceback]
    print(errmsg.format(*args))
