import enum
import json
import os
from pathlib import Path


def load_json_enums(in_: enum._EnumDict, filepath: str):
    with open(filepath, "r") as f:
        extra = json.load(f)
        for key, value in extra.items():
            in_[key] = value


class JsonFileChoicesMetaMixin:
    """load enum options from external json file"""

    @classmethod
    def __prepare__(metacls, cls, bases):
        classdict = metacls.__base__.__prepare__(cls, bases)
        # import pdb
        # pdb.set_trace()
        classdict._ignore.append("filepath")
        return classdict

    def __new__(metacls, classname, bases, classdict):
        filepath = classdict.get("filepath", "")
        basepath = Path(os.environ["SYS_BASE_PATH"]).resolve(strict=True)
        fullpath = os.path.join(basepath, filepath)
        load_json_enums(classdict, fullpath)
        return super().__new__(metacls, classname, bases, classdict)


class JsonFileChoicesMeta(JsonFileChoicesMetaMixin, enum.EnumMeta):
    pass


class AppCodeOptions(enum.Enum, metaclass=JsonFileChoicesMeta):
    filepath = "./common/data/app_code.json"


class ActivationStatus(enum.Enum):
    ACCOUNT_NON_EXISTENT = 1
    ACTIVATION_REQUEST = 2
    ACCOUNT_ACTIVATED = 3
    ACCOUNT_DEACTIVATED = 4
