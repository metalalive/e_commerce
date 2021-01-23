import json
import functools
import logging



def get_fixture_pks(filepath:str, pkg_hierarchy:str):
    assert pkg_hierarchy, "pkg_hierarchy must be fully-qualified model path"
    from django.conf  import  settings as django_settings
    from django.core.exceptions import ImproperlyConfigured
    preserved = None
    for d in django_settings.FIXTURE_DIRS:
        fixture_path = '/'.join([d, filepath])
        with open(fixture_path, 'r') as f:
            preserved = json.load(f)
        if preserved:
            break
    if not preserved:
        raise ImproperlyConfigured("fixture file not found, recheck FIXTURE_DIRS in settings.py")
    return  [str(item['pk']) for item in preserved if item['model'] == pkg_hierarchy]


def get_header_name(name:str):
    """
    normalize given input to valid header name that can be placed in HTTP request
    """
    from django.http.request    import HttpHeaders
    prefix = HttpHeaders.HTTP_PREFIX
    assert name.startswith(prefix),  "{} should have prefix : {}".format(name, prefix)
    out = name[len(prefix):]
    out = out.replace('_', '-')
    # print("get_header_name, out : "+ str(out))
    return out


def log_wrapper(logger, loglevel=logging.DEBUG):
    """
    log wrapper decorator for standalone functions e.g. async tasks,
    logging whenever the wrapped function reports error
    """
    def _wrapper(func):
        @functools.wraps(func) # copy metadata from custom func, which will be used for task caller
        def _inner(*arg, **kwargs):
            out = None
            log_args = ['action', func.__name__]
            try:
                out = func(*arg, **kwargs)
                log_args.extend(['status', 'completed', 'output', out])
                logger.log(loglevel, None, *log_args)
            except Exception as e:
                excpt_cls = "%s.%s" % (type(e).__module__ , type(e).__qualname__)
                excpt_msg = ' '.join(list(map(lambda x: str(x), e.args)))
                log_args.extend(['status', 'failed', 'excpt_cls', excpt_cls, 'excpt_msg', excpt_msg])
                logger.error(None, *log_args)
                raise
            return out
        return _inner
    return _wrapper


