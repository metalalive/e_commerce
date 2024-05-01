import os, logging, threading

_lock = threading.RLock()


def _acquireLock():
    """
    create module-level lock for serializing accesses to shared data,
    as coded in logging._acquireLock
    """
    if _lock:
        _lock.acquire()


def _releaseLock():
    if _lock:
        _lock.release()


class ExtendedLogger(logging.Logger):
    """
    add few argument(s) from Django application on calling log function :
        * request:
            Django request object, this logger extract client IP, request method, and URI
            from this object, and parse them to message format string
    """

    def _log(
        self,
        level,
        msg,
        args,
        exc_info=None,
        extra=None,
        stack_info=False,
        stacklevel=1,
        request=None,
    ):
        """
        override this internal method, to parse extra information associated with given HTTP request
        """
        if msg is None:
            # the key-value pairs are well-formed JSON, value is enclosed in double quotes
            msg = ['{"key":"%s", "value":"%s"}'] * (len(args) >> 1)
            msg = ",".join(msg)
            msg = "[%s]" % msg

        if request:
            client_ip = request.META.get("REMOTE_ADDR", "0.0.0.0")
            _extra = {
                "req_ip": client_ip,
                "req_mthd": request.method,
                "uri": request.get_full_path(),
            }
            if extra is not None:
                extra.update(_extra)
            else:
                extra = _extra
        # since I override the function that checks file path, line number from stack frame,
        # stack level must be added by one
        stacklevel += 1

        super()._log(
            level=level,
            msg=msg,
            args=args,
            exc_info=exc_info,
            extra=extra,
            stack_info=stack_info,
            stacklevel=stacklevel,
        )

    def findCaller(self, stack_info=False, stacklevel=1):
        stacklevel += 1
        rv = super().findCaller(stack_info=stack_info, stacklevel=stacklevel)
        rv = list(rv)
        base_path = type(self).sys_base_path
        fullpath = rv[0]
        if fullpath.find(base_path, 0, len(fullpath)) == 0:
            rv[0] = fullpath[len(base_path) :]
        elif fullpath.startswith("."):
            # some python applications (e.g. Django Daphne) converts the absolute path to
            # relative file path automatically (the string starts with `.`), which cause
            # Logstash parsing error because there is no Grok pattern that allows
            # relative file path , so here is a workaround for such case.
            rv[0] = fullpath[1:]
        rv = tuple(rv)
        return rv

    @classmethod
    @property
    def sys_base_path(cls):
        if not hasattr(cls, "_sys_base_path"):
            _acquireLock()
            cls._sys_base_path = os.environ.get("SERVICE_BASE_PATH", ".")
            _releaseLock()
        return cls._sys_base_path


logging.setLoggerClass(ExtendedLogger)
