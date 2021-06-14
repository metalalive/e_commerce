import os
import json
import string
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
    translate given key in Django request.META to header name in HTTP form
    """
    from django.http.request    import HttpHeaders
    prefix = HttpHeaders.HTTP_PREFIX
    if name.startswith(prefix): # skip those in HttpHeaders.UNPREFIXED_HEADERS
        name = name[len(prefix):]
    out = name.replace('_', '-').lower()
    # print("get_header_name, out : "+ str(out))
    return out


def get_request_meta_key(header_name:str):
    """
    translate given header name in HTTP form to accessible key in Django request.META
    """
    from django.http.request    import HttpHeaders
    out = header_name.replace('-', '_').upper()
    if out not in HttpHeaders.UNPREFIXED_HEADERS:
        out = '%s%s' % (HttpHeaders.HTTP_PREFIX, out)
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


def serial_kvpairs_to_dict(serial_kv:str, delimiter_pair=',', delimiter_kv=':'):
    outlst = []
    outdict = {}
    kv_pairs = serial_kv.split(delimiter_pair)
    for kv in kv_pairs:
        if delimiter_kv in kv:
            kv_lst = kv.split(delimiter_kv, 1)
            kv_lst = list(map(str.strip, kv_lst))
            outlst.append(kv_lst)
    #print('outlst = %s' % outlst)
    for pair in outlst:
        key = pair[0]
        if outdict.get(key, None) is None:
            outdict[key] = [pair[1]]
        else:
            outdict[key].append(pair[1])
    return outdict


def accept_mimetypes_lookup(http_accept:str, expected_types):
    # note that this is case-insensitive lookup
    client_accept = http_accept.split(',')
    client_accept = list(map(lambda x: x.strip().lower(), client_accept))
    expected_types = list(map(lambda x: x.strip().lower(), expected_types))
    result = filter(lambda x: x in expected_types, client_accept)
    return list(result)


def _get_amqp_url(secrets_path, idx=0):
    # use rabbitmqctl to manage accounts
    secrets = None
    with open(secrets_path, 'r') as f:
        secrets = json.load(f)
        secrets = secrets['amqp_broker']
        secrets = secrets[idx] # the default is to select index 0 as guest account (with password)
    assert secrets, "failed to load secrets from file"
    protocol = secrets['protocol']
    username = secrets['username']
    passwd = secrets['password']
    host   = secrets['host']
    port   = secrets['port']
    out = '%s://%s:%s@%s:%s' % (protocol, username, passwd, host, port)
    return out

def merge_partial_dup_listitem(list_in, combo_key, merge_keys):
    inter_dict = {}
    for old_item in list_in:
        inter_key = tuple([old_item.pop(k) for k in combo_key])
        for mk in merge_keys:
            if inter_dict.get(inter_key, None) is None:
                inter_dict[inter_key] = {}
            if inter_dict[inter_key].get(mk, None) is None:
                inter_dict[inter_key][mk] = []
            value = old_item.pop(mk, None)
            if value:
                inter_dict[inter_key][mk].append(value)
    list_in.clear()
    for inter_key, inter_value in inter_dict.items():
        new_item = {}
        for idx in range(len(inter_key)):
            new_item[combo_key[idx]] = inter_key[idx]
        for mk, merged_value in inter_value.items():
            new_item[mk] = merged_value
        list_in.append(new_item)


def string_unprintable_check(value:str, extra_unprintable_set=None):
    """
    this function returns any unprintable characters in the given
    input string, application callers can optionally add more characters
    which are not allowed to print in their usage context.
    """
    extra_unprintable_set = extra_unprintable_set or []
    _printable_char_set = set(string.printable) - set(extra_unprintable_set)
    generator = filter(lambda x: not x in _printable_char_set, value)
    return list(generator)


def format_sqlalchemy_url(driver:str, db_credential):
    """ format URL string used in SQLalchemy """
    url_pattern = '{db_driver}://{username}:{passwd}@{db_host}:{db_port}/{db_name}'
    return url_pattern.format(
            db_driver=driver ,
            username=db_credential['USER'] ,
            passwd=db_credential['PASSWORD'] ,
            db_host=db_credential['HOST'] ,
            db_port=db_credential['PORT'] ,
            db_name=db_credential['NAME'] ,
        )


class ExtendedDict(dict):
    def __init__(self, *args, **kwargs):
        self._modified = False
        super().__init__(*args, **kwargs)

    def __setitem__(self, key, value):
        self._modified = True
        super().__setitem__(key, value)

    @property
    def modified(self):
        return self._modified

    def update(self, new_kwargs, overwrite=True):
        if overwrite is False:
            common_keys = set(self.keys()) & set(new_kwargs.keys())
            new_kwargs = {k:v for k,v in new_kwargs.items() if k not in common_keys}
        if any(new_kwargs):
            self._modified = True
        return super().update(new_kwargs)
## end of class ExtendedDict



# class inheritance doesn't seem to work in metaclass
class BaseLookupMeta(type):
    def getitem(cls, key, _dict_name):
        item = None
        try:
            _dict = getattr(cls, _dict_name)
            item = _dict[key]
        except (KeyError, AttributeError) as e:
            err_msg = '[%s] %s when searching in %s, no such key : %s' % \
                (cls.__name__ , type(e), _dict, key)
            print(err_msg) # TODO, log to local file system
        return item


class BaseUriLookup(metaclass=BaseLookupMeta):
    _urls = {}

    def __getitem__(cls, key):
        return type(cls).getitem(key, '_urls') # first argument implicitly sent by type(cls)

    def __iter__(cls):
        cls._iter_api_url = iter(cls._urls)
        return cls

    def __next__(cls):
        key = next(cls._iter_api_url)
        return (key, cls._urls[key])


class BaseTemplateLookup(metaclass=BaseLookupMeta):
    _template_names = {}
    template_path  = ''

    def __getitem__(cls, key):
        item = BaseLookupMeta.getitem(cls, key, '_template_names')
        if item is not None:
            if isinstance(item, str):
                item = os.path.join(cls.template_path, item)
            elif isinstance(item, list):
                item = [os.path.join(cls.template_path, x) for x in item]
        return item


def monkeypatch_typing_specialform():
    """
    For alpha version of python (3.9.0a5) , the special typing class is
    not consistent with `typing_extensions` package , so it will lead to
    argument error when `_TypeAliasForm` class in `typing_extensions.py` is
    used as decorator and instantiated with non-existent argument `doc` in
    `typing._SpecialForm` which is internal class used in python standard
    library
    """
    import sys
    if sys.version_info[:2] == (3, 9) and sys.version_info.releaselevel == 'final':
        # no need to monkey-patch , for release version
        return
    from typing import _SpecialForm
    origin_getitem = _SpecialForm.__getitem__
    def patch_init(self, name, doc=None):
        if doc is None and callable(name):
            if hasattr(name, '__doc__'):
                doc = name.__doc__
        self._name = name
        self._doc = doc
    def patch_getitem(self, parameters):
        item = None
        if callable(self._name):
            item = self._name(self=self, parameters=parameters)
        else:
            item = origin_getitem(self=self, parameters=parameters)
        return item

    if not hasattr(_SpecialForm.__init__, 'patched'):
        _SpecialForm.__init__ = patch_init
        setattr(_SpecialForm.__init__, 'patched', True)
    if not hasattr(_SpecialForm.__getitem__, 'patched'):
        _SpecialForm.__getitem__ = patch_getitem
        setattr(_SpecialForm.__getitem__, 'patched', True)

