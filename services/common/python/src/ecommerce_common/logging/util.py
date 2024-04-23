import functools
import logging


def log_fn_wrapper(logger, loglevel=logging.DEBUG, log_if_succeed=True):
    """
    log wrapper decorator for standalone functions e.g. async tasks,
    logging whenever the wrapped function reports error
    """

    def _wrapper(func):
        @functools.wraps(
            func
        )  # copy metadata from custom func, which will be used for task caller
        def _inner(*arg, **kwargs):
            out = None
            log_args = ["action", func.__name__]
            try:
                out = func(*arg, **kwargs)
                log_args.extend(["status", "completed", "output", out])
                if log_if_succeed:
                    logger.log(loglevel, None, *log_args)
            except Exception as e:
                excpt_cls = "%s.%s" % (type(e).__module__, type(e).__qualname__)
                excpt_msg = " ".join(list(map(lambda x: str(x), e.args)))
                log_args.extend(
                    ["status", "failed", "excpt_cls", excpt_cls, "excpt_msg", excpt_msg]
                )
                logger.error(None, *log_args)
                raise
            return out

        return _inner

    return _wrapper
