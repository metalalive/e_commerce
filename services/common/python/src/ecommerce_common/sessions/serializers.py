from datetime import datetime
import json
import logging

_logger = logging.getLogger(__name__)


class ExtendedJSONSerializer:
    """
    add functions to :
    * serialize datetime object
    """

    @staticmethod
    def _encode_extra_types(obj):
        if isinstance(obj, datetime):
            return {"__datetime__": obj.isoformat()}
        log_args = ["msg", "unable to serialize this data type", "obj", type(obj)]
        _logger.error(None, *log_args)
        raise TypeError(type(obj))

    @staticmethod
    def _decode_extra_types(d):
        if "__datetime__" in d:
            d = datetime.fromisoformat(d["__datetime__"])
        return d

    def dumps(self, obj):
        return json.dumps(
            obj, default=self._encode_extra_types, separators=(",", ":")
        ).encode("latin-1")

    def loads(self, data):
        return json.loads(data.decode("latin-1"), object_hook=self._decode_extra_types)
