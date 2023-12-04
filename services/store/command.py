import sys
import inspect
from migrations.alembic.config import downgrade_migration, auth_provider_upgrade, resource_app_upgrade
from .settings import migration as settings


def auth_migrate_forward (args):
    auth_curr_rev_id = '000002'
    auth_provider_upgrade (app_settings=settings,
            init_round=True, next_rev_id=auth_curr_rev_id)

def store_migrate_forward (args):
    assert len(args) == 3
    dependent_rev_id = None if args[0].lower() == 'init'  else [args[0]]
    kwargs = {'app_settings':settings, 'next_rev_id':args[1],
            'new_label':args[2], 'dependent_rev_id':dependent_rev_id }
    resource_app_upgrade ( **kwargs )

def migrate_backward (args):
    assert len(args) == 1
    prev_rev_id = 'base' if args[0].lower() == 'init' else args[0] 
    kwargs = {'app_settings':settings, 'prev_rev_id':prev_rev_id }
    downgrade_migration( **kwargs )

if __name__ == "__main__":
    assert len(sys.argv) >= 2
    curr_mod = sys.modules[__name__]
    members = inspect.getmembers(curr_mod, inspect.isfunction)
    # skip sys.argv[0] which indicates path to this file
    chosen = filter(lambda label_and_fn: label_and_fn[0] == sys.argv[1]  , members)
    _ , fn = next(chosen)
    fn(sys.argv[2:])

