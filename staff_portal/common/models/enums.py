import enum
import json
from django.db.models.enums import IntegerChoices, ChoicesMeta


class TupleChoicesMeta(ChoicesMeta):
    """
    this choice class always selects first item of a tuple as an option of enum type
    """
    @property
    def choices(cls):
        empty = [(None, cls.__empty__)] if hasattr(cls, '__empty__') else []
        return empty + [(member.value[0][0], member.label) for member in cls]


def load_json_enums(in_:enum._EnumDict, filepath:str):
    with open(filepath, 'r') as f:
        extra = json.load(f)
        for key, value in extra.items():
            in_[key] = value


class JsonFileChoicesMeta(ChoicesMeta):
    """ load enum options from external json file  """
    @classmethod
    def __prepare__(metacls, cls, bases):
        classdict =  ChoicesMeta.__prepare__(cls, bases)
        classdict._ignore.append('filepath')
        return classdict

    def __new__(metacls, classname, bases, classdict):
        filepath = classdict.get('filepath', '')
        load_json_enums(classdict, filepath)
        return super().__new__(metacls, classname, bases, classdict)


class UnitOfMeasurement(IntegerChoices, metaclass=JsonFileChoicesMeta):
    """
    unit(countable object) 1 - 2
    working time,      65  - 71
    weight,            129 - 140
    length / distance, 193 - 202
    volume (liquid),   256 -
    """
    filepath = './common/data/unit_of_measurement.json'


