from datetime import timedelta, datetime, date, UTC
from types import GeneratorType
import os
import random
import logging
import uuid
import json

from ecommerce_common.util import string_unprintable_check

_logger = logging.getLogger(__name__)


class AbstractKeystorePersistReadMixin:
    def __len__(self):
        raise NotImplementedError

    def __getitem__(self, key_id):
        """
        look for valid crypto-key item, clone it then return
        """
        raise NotImplementedError


class AbstractCryptoKeyPersistHandler(AbstractKeystorePersistReadMixin):
    MAX_EXPIRED_AFTER_DAYS = 365
    DEFAULT_EXPIRED_AFTER_DAYS = 30
    FLUSH_THRESHOLD_NUM_ITEMS = 550
    """
    the structure for the crypto-key in the data source may be like this :
    {
        "crypto-key-id-5678": {
            'exp': 'EXPIRY_DATE_IN_ISO_STRING_FORMAT',
            'alg': 'ALGORITHM_FOR_THE_KEY',
            'kty': 'CRYPTO_KEY_TYPE',
            'use': 'HOW_DO_YOU_USE_THE_KEY',
        },
        "crypto-key-id-9012": {
            'exp': 'EXPIRY_DATE_IN_ISO_STRING_FORMAT',
            'alg': 'ALGORITHM_FOR_THE_KEY',
            'kty': 'CRYPTO_KEY_TYPE',
            'use': 'HOW_DO_YOU_USE_THE_KEY',
        },
        "crypto-key-id-3456": {
            'exp': 'EXPIRY_DATE_IN_ISO_STRING_FORMAT',
            'alg': 'ALGORITHM_FOR_THE_KEY',
            'kty': 'CRYPTO_KEY_TYPE',
            'use': 'HOW_DO_YOU_USE_THE_KEY',
        }
        .....
    }
    """
    VALID_FIELDS = ["exp", "alg", "kty", "use"]

    def __init__(
        self,
        name="default persist handler",
        expired_after_days=DEFAULT_EXPIRED_AFTER_DAYS,
        max_expired_after_days=MAX_EXPIRED_AFTER_DAYS,
        flush_threshold=FLUSH_THRESHOLD_NUM_ITEMS,
        auto_flush=True,
    ):
        self.name = name
        self.expired_after_days = expired_after_days
        self.max_expired_after_days = max_expired_after_days
        self._flush_threshold_num_items = flush_threshold
        self._auto_flush = auto_flush
        self._uncommitted_add = {}
        self._uncommitted_delete = set()  # []

    @property
    def expired_after_days(self):
        """expiry time delta in days"""
        return self._expired_after_days

    @expired_after_days.setter
    def expired_after_days(self, value):
        assert value > 0 and isinstance(value, int), (
            "expired_after_days has to be positive integer, but receive %s" % value
        )
        self._expired_after_days = timedelta(days=value)

    @property
    def auto_flush(self):
        return self._auto_flush

    @auto_flush.setter
    def auto_flush(self, value: bool):
        self._auto_flush = value

    @property
    def flush_threshold(self):
        return self._flush_threshold_num_items

    @flush_threshold.setter
    def flush_threshold(self, value: int):
        self._flush_threshold_num_items = value

    def __len__(self):
        """subclasses should return number of crypto keys available"""
        num_keys = 0
        key_ids = self.iterate_key_ids()
        if isinstance(key_ids, GeneratorType):
            try:
                while next(key_ids):
                    num_keys += 1
            except StopIteration:
                pass
        elif hasattr(key_ids, "__iter__") and callable(key_ids.__iter__):
            num_keys = len(key_ids)
        else:
            raise NotImplementedError
        return num_keys

    def _set_item_error_check(self, key_id, item):
        min_field_cover = set(self.VALID_FIELDS) - set(item.keys())
        if any(min_field_cover):
            errmsg = (
                "The fields of key item should cover the minimum set %s , but found %s"
                % (self.VALID_FIELDS, min_field_cover)
            )
            raise ValueError(errmsg)
        expiry_user = date.fromisoformat(item["exp"])
        expiry_not_exceed = date.today() + timedelta(days=self.max_expired_after_days)
        if expiry_user > expiry_not_exceed:
            errmsg = (
                "user-specified expiry date %s exceeds maximum allowed value %s"
                % (expiry_user, expiry_not_exceed)
            )
            raise ValueError(errmsg)
        extra_unprintable_set = [
            "\x09",
            "\x08",
            "\x0b",
            "\x0c",
            "\x0d",
            '"',
        ]  # new-line char can be written to file line
        if string_unprintable_check(
            key_id, extra_unprintable_set=extra_unprintable_set
        ):
            raise ValueError("the key_id is not printable : `%s`" % key_id)
        for value in item.values():
            if isinstance(value, str) and string_unprintable_check(
                value, extra_unprintable_set=extra_unprintable_set
            ):
                raise ValueError("the value is not printable : `%s`" % value)

    def __setitem__(self, key_id, item: dict):
        self._set_item_error_check(key_id=key_id, item=item)
        key_ids = self.iterate_key_ids()
        if key_id in key_ids:
            self._remove(key_ids=[key_id])
        self._uncommitted_add[key_id] = item
        self._flush_if_full()

    def remove(self, key_ids):
        self._remove(key_ids=key_ids)
        self._flush_if_full()

    def _remove(self, key_ids):
        key_ids = key_ids or []
        self._uncommitted_delete = self._uncommitted_delete | set(key_ids)

    def evict_expired_keys(self, date_limit=None):
        result = []  # used to log evicted key items
        evict = []
        present_fields = ["exp"]
        date_limit = date_limit or date.today()
        for kid, value in self.items(present_fields=present_fields):
            expiry = date.fromisoformat(value["exp"])
            if date_limit > expiry:
                evict.append(kid)
                # DO NOT expose secrets to external logging service
                log_item = {
                    "persist_handler": self.name,
                    "kid": kid,
                    "exp": value["exp"],
                }
                result.append(log_item)
        # evict by key ids, and might be flushed if there are too many deleting items
        self.remove(key_ids=evict)
        return result

    @property
    def is_full(self):
        num_adding = len(self._uncommitted_add.keys())
        num_deleting = len(self._uncommitted_delete)
        return (num_adding + num_deleting) >= self.flush_threshold

    def _flush_if_full(self):
        if self.is_full and self.auto_flush:
            self.flush()
        else:
            # TODO, log if number of uncommitted items don't meet the threshold
            pass  # skip , application callers may need to force it flush later

    def flush(self):
        """
        flush the uncommitted change (added or deleted crypo-key items)
        to destination declared by subclasses .
        """
        raise NotImplementedError

    def iterate_key_ids(self):
        """
        this function iterates all crypto-key IDs from source (defined in
        subclasses) and return each of the IDs
        """
        raise NotImplementedError

    def items(self, present_fields=None):
        """
        this function iterates all crypto-key items from source (defined in
        subclasses) and return crypto-key Id and the associated item as
        key-value pairs .
        """
        raise NotImplementedError

    def random_choose(self):
        key_ids = self.iterate_key_ids()
        key_id = None
        # When the key_ids is a generator, it doesn't seem efficient to
        # load all of the items returned by the generator especially if
        # the generator will produce millions of items (e.g. read from
        # big data source) , instead my approach here is to use a mini-function
        # which returns true or false randonly.
        whether_to_take = lambda x: (random.randrange(x) & 0x1) == 0x1
        # Every time the generator returns an item, the mini-function
        # helps to decide whether to take this item or not, so I can take
        # any subsequent item randonly.
        for k in key_ids:
            if key_id is None:
                key_id = k
            elif whether_to_take(0xFFFF):
                key_id = k
                break
        item = self[key_id]
        # should return extra field `kid`
        item["kid"] = key_id
        return item


# end of class AbstractCryptoKeyPersistHandler


class JWKSFilePersistHandler(AbstractCryptoKeyPersistHandler):
    """
    This class is aimed at persisting cryptography keys in JWKS form
    (JSON Web Keys Set) to OS file system, the python application having
    this class should also have read / write permission to target
    file directory.
    This class works properly with limited internal file structure:
        * first line has to be single left  curly bracket `{`
        * last  line has to be single right curly bracket `}`
        * each line between first line and last line represents raw data
          of key-value pair, whose the value part is a nested JSON object
    """

    NUM_BACKUP_FILES_KEPT = 5
    JSONFILE_START_LINE = "{\n"  # left  curly bracket
    JSONFILE_END_LINE = "}\n"  # right curly bracket

    def __init__(self, filepath, **kwargs):
        super().__init__(**kwargs)
        import ijson

        self._ijson = ijson
        self._file = open(filepath, mode="rb")

    def __del__(self):
        if hasattr(self, "_file") and self._file and not self._file.closed:
            self._file.close()

    def flush(self):
        if not self._uncommitted_add and not self._uncommitted_delete:
            return
        tmp_wr_file_name = "%s.new" % self._file.name
        pos_add = 0
        prev_wr_rawline = ""
        prev_wr_file_pos = 0
        # no need to apply lock, in this project there should NOT be seperate python
        # processes flushing to the same file
        with open(tmp_wr_file_name, mode="w") as tmp_wr_file:
            self._file.seek(0)
            # telling position tell() on read file is disabled when iterating
            # each line in the file object
            for rawline in self._file:
                # TODO / FIXME, figure out why file object returns string line even I
                # explicitly set it to read-byte mode
                rline = (
                    str(rawline, encoding="utf-8")
                    if isinstance(rawline, bytes)
                    else rawline
                )
                if rline == self.JSONFILE_START_LINE:
                    prev_wr_rawline = rline
                    prev_wr_file_pos = tmp_wr_file.tell()
                    tmp_wr_file.write(rline)
                elif rline == self.JSONFILE_END_LINE:  # adjust comma in last object
                    self._adjust_comma_on_flush_deletion(
                        wr_file=tmp_wr_file,
                        prev_wr_file_pos=prev_wr_file_pos,
                        prev_wr_rawline=prev_wr_rawline,
                    )
                else:
                    rawkey = rline.split(":")[0]
                    # assert len(rawkey) >= 2 , "the key-value pair has to be stored in one line"
                    rawkey = rawkey.strip()
                    left_quote_pos = rawkey.find(
                        '"', 0
                    )  # has to be positional arguments
                    right_quote_pos = rawkey.find('"', left_quote_pos + 1)
                    key_id = rawkey[left_quote_pos + 1 : right_quote_pos]
                    if key_id in self._uncommitted_delete:
                        pass  # delete the object by NOT writing the line to new file
                    else:
                        prev_wr_rawline = rline
                        prev_wr_file_pos = (
                            tmp_wr_file.tell()
                        )  # record current position before writing this line
                        tmp_wr_file.write(rline)
            self._add_items_on_flush(wr_file=tmp_wr_file)  # add new objects
            tmp_wr_file.write(self.JSONFILE_END_LINE)
        self._switch_files_on_flush(wr_file_path=tmp_wr_file_name)
        self._uncommitted_delete.clear()
        self._uncommitted_add.clear()

    ## end of flush()

    def _adjust_comma_on_flush_deletion(
        self, wr_file, prev_wr_file_pos, prev_wr_rawline
    ):
        pos_end_of_map = prev_wr_rawline.rfind("}")
        if pos_end_of_map <= 0:
            return  # object not found in previous line, it may be empty json file
        comma = prev_wr_rawline[pos_end_of_map + 1 : -1]
        comma = comma.strip()
        edited_prev_rawline = []
        # add comma symbol `,` if there are new objects
        if self._uncommitted_add and comma != ",":
            edited_prev_rawline = [prev_wr_rawline[: pos_end_of_map + 1], ",", "\n"]
        elif not self._uncommitted_add and comma == ",":
            edited_prev_rawline = [prev_wr_rawline[: pos_end_of_map + 1], "\n"]
        if edited_prev_rawline:
            prev_wr_rawline = "".join(edited_prev_rawline)
            wr_file.seek(prev_wr_file_pos)
            wr_file.write(prev_wr_rawline)

    def _add_items_on_flush(self, wr_file):
        for key, value in self._uncommitted_add.items():
            serial_v = json.dumps(value)
            serialized = '"%s":%s,\n' % (key, serial_v)
            wr_file.write(serialized)
        if self._uncommitted_add:
            wr_pos = wr_file.tell()
            wr_file.seek(wr_pos - 2)
            wr_file.write("\n")  # remove the comma in the last object

    def _switch_files_on_flush(self, wr_file_path):
        """
        rename current version file to old_<DATETIME>_<OLD_FILE_NAME> for
        backup purpose, then rename new version file to <OLD_FILE_NAME>
        then the whole system will use rotated crypto-key set after this
        function completes its task.
        """
        nowtime = datetime.now(UTC).isoformat()
        rd_file_path = self._file.name
        rd_file_dir = os.path.dirname(rd_file_path)
        rd_file_name = os.path.basename(rd_file_path)
        current_version_filename_from = rd_file_path
        current_version_filename_to = "%s/old_%s_%s" % (
            rd_file_dir,
            nowtime,
            rd_file_name,
        )
        next_version_filename_from = wr_file_path
        next_version_filename_to = rd_file_path
        # no need to apply lock,  there should NOT be seperate python
        # processes renaming the same file
        self._file.close()
        os.rename(current_version_filename_from, current_version_filename_to)
        os.rename(next_version_filename_from, next_version_filename_to)
        self._clean_old_backup(file_dir=rd_file_dir)
        self._file = open(self._file.name, mode="r")

    def _clean_old_backup(self, file_dir):
        fullpaths = map(
            lambda fname: os.path.join(file_dir, fname), os.listdir(file_dir)
        )
        filenames = filter(
            lambda fname: os.path.isfile(fname)
            and fname != self._file.name
            and not os.path.basename(fname).startswith("."),
            fullpaths,
        )
        filenames = list(filenames)
        filenames.sort(key=lambda fname: os.path.getmtime(fname), reverse=True)
        # only keep most recent few files
        delete_files = filenames[self.NUM_BACKUP_FILES_KEPT :]
        list(map(os.remove, delete_files))

    def iterate_key_ids(self):
        self._file.seek(0)
        parse_evts = self._ijson.parse(self._file)
        for prefix, evt_label, value in parse_evts:
            if prefix == "" and evt_label == "map_key":
                yield value
        # update key ID list for any difference

    def items(self, present_fields=None):
        self._file.seek(0)
        present_fields = present_fields or []
        parse_evts = self._ijson.parse(self._file)
        key_id = ""
        tmp_field_name = ""
        yld_item = {}

        for prefix, evt_label, value in parse_evts:
            if evt_label == "map_key":
                if prefix == "":
                    key_id = value
                elif prefix == key_id and value in present_fields:
                    tmp_field_name = value
            elif evt_label in ["string", "number"]:
                if prefix == "%s.%s" % (key_id, tmp_field_name):
                    yld_item[tmp_field_name] = value
            elif evt_label == "end_map":
                if key_id and prefix == key_id:
                    yield key_id, yld_item  # TODO:clone the item or not ?
                    # callers cannot use dict() or tuple() etc. to fetch all the data
                    yld_item.clear()

    def __getitem__(self, key_id):
        # currently this function limits to fetch those items which are already committed & stored
        item = None
        self._file.seek(0)
        generator = self._ijson.items(self._file, key_id)
        try:
            item = next(generator)
        except StopIteration:
            raise KeyError("invalid key ID : %s" % key_id)
        try:
            dup_kid_item = next(generator)
            raise ValueError("duplicate key ID : %s" % key_id)
        except StopIteration:
            pass  # the item with unique key ID should go here
        return item.copy()


# end of class JWKSFilePersistHandler


class AbstractKeygenHandler:
    @property
    def key_type(self):
        raise NotImplementedError

    @property
    def algorithm(self):
        raise NotImplementedError

    @property
    def asymmetric(self):
        raise NotImplementedError

    def generate(self, key_size_in_bits, **kwargs):
        raise NotImplementedError


class RSAKeygenHandler(AbstractKeygenHandler):
    def __init__(self, **kwargs):
        from c_exts.util import keygen

        self._keygen = keygen

    @property
    def key_type(self):
        return "RSA"

    @property
    def algorithm(self):
        return "RSA"

    @property
    def asymmetric(self):
        return True

    def generate(self, key_size_in_bits, num_primes=2):
        """
        the RSA algorithm defaults to 2 primes , `num_primes` must NOT less than 2,
        also multi-prime RSA key (`num_primes` > 2) has potential risk of being
        compromised faster than typical 2-prime RSA key if application doesn't
        appropriately configure it.
        """
        return self._keygen.RSA_keygen(key_size_in_bits, num_primes)


class BaseAuthKeyStore:
    DEFAULT_NUM_KEYS = 2
    # For the members which are associated with crypto key in application,
    # such members should be added by concrete subclasses of AbstractKeygenHandler
    _key_item_template = {
        field_name: None for field_name in AbstractCryptoKeyPersistHandler.VALID_FIELDS
    }

    def __init__(self, persist_secret_handler=None, persist_pubkey_handler=None):
        # persist_pubkey_handler could be ignored if application callers apply symmetric-key algorithm
        self._persistence = {
            "secret": persist_secret_handler,
            "pubkey": persist_pubkey_handler,
        }

    def _check_persist_secret_handler_exists(self):
        assert (
            self._persistence["secret"] and len(self._persistence["secret"]) > 0
        ), "Handler for persisting secrets has to be provided, it should also contain at least one key"

    def _check_persist_pubkey_handler_exists(self):
        assert (
            self._persistence["pubkey"] and len(self._persistence["pubkey"]) > 0
        ), "Handler for persisting public keys has to be provided, it should also contain at least one key."

    def _construct_serializable_keyitem(
        self, persist_handler, kid, keyparser, alg, date_start, use, kty
    ):
        new_item = self._key_item_template.copy()
        new_item["use"] = use
        new_item["kty"] = kty
        new_item["alg"] = alg
        expiry = date_start + persist_handler.expired_after_days
        new_item["exp"] = expiry.isoformat()
        keyparser(new_item)
        try:
            persist_handler[kid] = new_item
            result = {
                "kid": kid,
                "alg": new_item["alg"],
                "exp": new_item["exp"],
                "persist_handler": persist_handler.name,
            }
        except KeyError as e:  # collision happens on key id
            result = None  # overwrite existing item is NOT allowed, try another key id
        return result

    def _gen_keys(
        self, keygen_handler, num_keys_required, date_start, key_size_in_bits
    ):
        out = []
        next_num_keys = num_keys_required
        num_valid_secrets = len(self._persistence["secret"]) - len(
            self._persistence["secret"]._uncommitted_delete
        )
        if keygen_handler.asymmetric:
            num_valid_pubkeys = len(self._persistence["pubkey"]) - len(
                self._persistence["pubkey"]._uncommitted_delete
            )
            curr_num_keys = min(num_valid_pubkeys, num_valid_secrets)
        else:
            curr_num_keys = num_valid_secrets
        if next_num_keys <= curr_num_keys:
            log_item = {
                "next_num_keys": next_num_keys,
                "curr_num_keys": curr_num_keys,
                "msg": "no new key generated",
                "action": "generate",
            }
            out.append(log_item)
        else:
            next_num_keys = next_num_keys - curr_num_keys
            for idx in range(next_num_keys):
                keyset = keygen_handler.generate(key_size_in_bits=key_size_in_bits)
                result = None
                while (
                    not result
                ):  # unlinkely to stuck at this loop, key_id collision happens rarely
                    kwargs_secret = {
                        "persist_handler": self._persistence["secret"],
                        "date_start": date_start,
                        "alg": keyset.algorithm,
                        "keyparser": keyset.private,
                        "kid": str(uuid.uuid4()),
                        "use": "sig",
                        "kty": keygen_handler.key_type,
                    }
                    result = self._construct_serializable_keyitem(**kwargs_secret)
                    if result:
                        out.append(result)
                if (
                    keygen_handler.asymmetric
                ):  # key_id has to be consistent among both of the persist handlers
                    kwargs_pubkey = {
                        "persist_handler": self._persistence["pubkey"],
                        "date_start": date_start,
                        "alg": keyset.algorithm,
                        "keyparser": keyset.public,
                        "kid": result["kid"],
                        "use": "sig",
                        "kty": keygen_handler.key_type,
                    }
                    out.append(self._construct_serializable_keyitem(**kwargs_pubkey))
        return out

    def _evict_expired_keys(self, keygen_handler, date_limit):
        out = []
        result = self._persistence["secret"].evict_expired_keys(date_limit=date_limit)
        out.extend(result)
        if keygen_handler.asymmetric:
            result = self._persistence["pubkey"].evict_expired_keys(
                date_limit=date_limit
            )
            out.extend(result)
        return out

    def rotate(
        self,
        keygen_handler,
        key_size_in_bits,
        num_keys=DEFAULT_NUM_KEYS,
        date_limit=None,
    ):
        assert num_keys > 0, (
            "num_keys has to be positive integer, but gets %s" % num_keys
        )
        result = {"evict": None, "new": None}
        date_limit = date_limit or date.today()
        result["evict"] = self._evict_expired_keys(
            keygen_handler=keygen_handler, date_limit=date_limit
        )
        result["new"] = self._gen_keys(
            keygen_handler=keygen_handler,
            num_keys_required=num_keys,
            date_start=date_limit,
            key_size_in_bits=key_size_in_bits,
        )
        self._persistence["secret"].flush()
        if keygen_handler.asymmetric and self._persistence["pubkey"] is not None:
            self._persistence["pubkey"].flush()
        return result

    def _choose(self, persist_handler, kid, randonly):
        item = None
        if kid:
            item = persist_handler[kid]
        # TODO, how to detect if a key exists in the item while the field name is unknown ?
        # if not kid or (item and not item['key'] and randonly):
        if not kid or (not item and randonly):
            item = persist_handler.random_choose()  # should return extra field `kid`
        return item  # which contains `key` and `alg` fields

    def choose_pubkey(self, kid):
        assert kid, "`kid` has to be valid key identifier, but receive %s" % kid
        self._check_persist_pubkey_handler_exists()
        return self._choose(
            persist_handler=self._persistence["pubkey"], kid=kid, randonly=False
        )

    def choose_secret(self, kid=None, randonly=False):
        assert kid or randonly, "if kid is null, randonly has to be set `True`"
        self._check_persist_secret_handler_exists()
        item = self._choose(
            persist_handler=self._persistence["secret"], kid=kid, randonly=randonly
        )
        return item


## end of BaseAuthKeyStore


def create_keystore_helper(cfg, import_fn):
    ks_kwargs = {}
    keystore_cls = import_fn(cfg["keystore"])
    if cfg.get("persist_secret_handler", None):
        persist_handler_module = import_fn(cfg["persist_secret_handler"]["module_path"])
        persist_handler_kwargs = cfg["persist_secret_handler"].get("init_kwargs", {})
        ks_kwargs["persist_secret_handler"] = persist_handler_module(
            **persist_handler_kwargs
        )
    if cfg.get("persist_pubkey_handler", None):
        persist_handler_module = import_fn(cfg["persist_pubkey_handler"]["module_path"])
        persist_handler_kwargs = cfg["persist_pubkey_handler"].get("init_kwargs", {})
        ks_kwargs["persist_pubkey_handler"] = persist_handler_module(
            **persist_handler_kwargs
        )
    return keystore_cls(**ks_kwargs)
