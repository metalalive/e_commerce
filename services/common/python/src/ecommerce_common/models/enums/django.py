from django.db.models.enums import IntegerChoices, ChoicesMeta

from .base import JsonFileChoicesMetaMixin


class TupleChoicesMeta(ChoicesMeta):
    """
    this choice class always selects first item of a tuple as an option of enum type
    """

    @property
    def choices(cls):
        empty = [(None, cls.__empty__)] if hasattr(cls, "__empty__") else []
        return empty + [(member.value[0][0], member.label) for member in cls]


class JsonFileChoicesMeta(JsonFileChoicesMetaMixin, ChoicesMeta):
    pass


class UnitOfMeasurement(IntegerChoices, metaclass=JsonFileChoicesMeta):
    """
    unit(countable object) 1 - 2
    working time,      65  - 71
    weight,            129 - 140
    length / distance, 193 - 202
    volume (liquid),   256 -
    """

    filepath = "./common/data/unit_of_measurement.json"


class AppCodeOptions(IntegerChoices, metaclass=JsonFileChoicesMeta):
    filepath = "./common/data/app_code.json"
