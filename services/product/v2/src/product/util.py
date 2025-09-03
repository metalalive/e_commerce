import random
import string
from datetime import datetime
from dataclasses import dataclass
from enum import Enum
from typing import Dict, List, Optional, Self


def gen_random_string(max_length: int) -> str:
    t0 = datetime.now()
    random.seed(a=t0.timestamp())  # TODO, common function which generate random string
    characters = string.ascii_letters + string.digits
    return "".join(random.choices(characters, k=max_length))


def gen_random_number(num_bits: int) -> int:
    t0 = datetime.now()
    random.seed(a=t0.timestamp())
    return random.getrandbits(num_bits)


class PriviledgeLevel(Enum):
    AuthedUser = "authed_user"


class QuotaMaterialCode(Enum):
    NumSaleItem = 2
    NumAttributesPerItem = 3


@dataclass
class QuotaMaterial:
    mat_code: QuotaMaterialCode
    maxnum: Optional[int] = 0

    @classmethod
    def extract(cls, claims: Dict) -> List[Self]:
        from ecommerce_common.models.enums.base import AppCodeOptions

        return [
            cls(mat_code=QuotaMaterialCode(q["mat_code"]), maxnum=q["maxnum"])
            for q in claims["quota"]
            if q["app_code"] == AppCodeOptions.product.value[0]
        ]

    @staticmethod
    def find_maxnum(quotas: List[Self], code_req: QuotaMaterialCode) -> int:
        def filter_fn(q: Self) -> int:
            return q.mat_code == code_req

        quota = next(filter(filter_fn, quotas), None)
        if quota:
            return quota.maxnum
        else:
            return 0


"""
[Note]
permission check / authorization logic is collected in middleware not in
`guardpost.Policy` or `guardpost.authorization.AuthorizationContext` or
`guardpost.authorization.Requirement` , because these classes do not contain
any reference to incoming HTTP request ; This application does require to check
fields in HTTP request when checking permissions on specific endpoint.
"""


def permission_check(claims: Dict, required: List[str]) -> Optional[Dict]:
    from ecommerce_common.models.enums.base import AppCodeOptions

    perms_approved = [
        p["codename"] for p in claims["perms"] if p["app_code"] == AppCodeOptions.product.value[0]
    ]
    perms_missing = set(required) - set(perms_approved)
    if perms_missing:
        return {"missing_permissions": list(perms_missing)}
    else:
        return None
