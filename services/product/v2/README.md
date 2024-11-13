# Product service

## Pre-requisite
| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.12.0 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|Poetry| 1.8.4 | [see here](https://python-poetry.org/docs) |
|pip| 24.3.1 | [see here](https://pip.pypa.io/en/stable/) |

## Build
### Dependency update
Update all dependency packages specified in the configuration file `v1.0.1/pyproject.toml`
```bash
poetry update
```
To update specific dependency downloaded from pip server :
```bash
poetry update <WHATEVER-3RD-PARTY-PACKAGE-NAME>
```

To update local dependency `ecommerce-common` :
- comment the dependency setup in `pyproject.toml`,
- run `poetry update`
- uncomment the dependency setup in `pyproject.toml`,
- run `poetry update` again

These steps does not seem efficient, but it does force the update, if you simply run `poetry update ecommerce-common` with version change, then poetry will internally ignore the update without any hint / warning message.

### specify local paths to source code packages
It is essential to run `install` command to let virtual environment know the local paths to source code packages
```bash
poetry install
```
After that you should be able to import the packages of the development code
```bash
poetry run python

> import sys
> sys.path
['/PATH/TO/PACKAGE1', '/PATH/TO/PROJ_HOME', '/PATH/TO/PROJ_HOME/src' ....]
> import product
> import settings
>
```


## Run
### application server
```bash
APP_SETTINGS="settings.development" poetry run granian --host 127.0.0.1 --port 8009 \
    --interface asgi  product.entry.web:app
```

## Test
```bash
APP_SETTINGS="settings.development" poetry run pytest 
```

## Development
### code formatter
```bash
poetry run black ./src/ ./tests/ ./settings/
```

### linter
```bash
poetry run ruff check ./src/ ./tests/ ./settings/
```

