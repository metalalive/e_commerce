# Store-Front service
## Build
### Database Migration
```bash
python3.9 -m  store.command  [subcommand] [args]
```

Where `subcommand` could be one of followings :
| `subcommand` | arguments | description |
|-|-|-|
|`auth_migrate_forward`| `N/A` | Always upgrade to latest revision provided by the codebase. Do NOT manually modify the default migration script in `/YOUR-PROJECT-HOME/migrations/alembic/store` |
|`store_migrate_forward`| `[prev-rev-id]  [new-rev-id]  [new-label]` | `[prev-rev-id]` is an alphanumeric byte sequence representing revision of current head in the migration history. <br><br> `[new-rev-id]` has the same format as `[prev-rev-id]` but it labels revision of the new head. <br><br> For initial migration upgrade, `[prev-rev-id]` has to be `init`. <br><br> Example : `python3 -m store.command store_migrate_forward  init  00001  init-app-tables` |
|`migrate_backward`| `[target-rev-id]` | `[target-rev-id]` is an alphanumeric byte sequence representing revision at previous point of the migration history. <br><br> To downgrade back to initial state `[target-rev-id]` can also be `init`, which removes all created schemas and entire migration folder.<br><br> Example : `python3 -m store.command  migrate_backward  00001` |
||||

## Run
### Development Server
```bash
APP_SETTINGS="store.settings.development" uvicorn  --host 127.0.0.1 \
    --port 8011 store.entry:app  >& ./tmp/log/dev/store_app.log &
```

### Production Server
(TODO)

## Test
### Unit Test
```bash
python3 -m unittest tests.python.util.rpc  -v
```

### Integration Test
```bash
APP_SETTINGS="store.settings.test" pytest -v -s --keepdb ./store/tests/storeprofile/models.py
APP_SETTINGS="store.settings.test" pytest -v -s --keepdb ./store/tests/storeprofile/api.py
APP_SETTINGS="store.settings.test" pytest -v -s --keepdb ./store/tests/staff.py
APP_SETTINGS="store.settings.test" pytest -v -s --keepdb ./store/tests/business_hours.py
APP_SETTINGS="store.settings.test" pytest -v -s --keepdb ./store/tests/products.py
```

