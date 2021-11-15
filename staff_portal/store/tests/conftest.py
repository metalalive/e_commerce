
def pytest_addoption(parser):
    parser.addoption('--keepdb', action='store_true', default=False,
            help='whether to keep database schema (not data itself) after tests ended')

