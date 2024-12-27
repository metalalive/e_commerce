import random
import string
from datetime import datetime


def gen_random_string(max_length: int) -> str:
    t0 = datetime.now()
    random.seed(a=t0.timestamp())  # TODO, common function which generate random string
    characters = string.ascii_letters + string.digits
    return "".join(random.choices(characters, k=max_length))


def gen_random_number(num_bits: int) -> int:
    t0 = datetime.now()
    random.seed(a=t0.timestamp())
    return random.getrandbits(num_bits)
