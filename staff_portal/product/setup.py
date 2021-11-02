import sys
import os
from pathlib import Path
import shutil

from django import setup
from django.core.management import call_command

from migrations.django  import auto_deploy


def init_migration():
    os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'product.settings.migration')
    setup()
    call_command('makemigrations', 'contenttypes')
    call_command('makemigrations', 'product')
    # --- schema migration ---
    import product
    apps = (product,)
    auto_deploy(apps)
    options = {'database':'site_dba',}
    call_command('migrate', 'contenttypes', **options)
    call_command('migrate', 'product', '0002', **options)
    options = {'database':'usermgt_service',}
    call_command('migrate', 'product', '0003', **options)


def deinit_migration():
    os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'product.settings.migration')
    setup()
    import product
    options = {'database':'usermgt_service',}
    call_command('migrate', 'product', '0002', **options)
    options = {'database':'site_dba',}
    call_command('migrate', 'product', 'zero', **options)
    call_command('migrate', 'contenttypes', 'zero', **options)
    apps = (product,)
    for app in apps:
        app_path = Path(app.__file__).resolve(strict=True)
        migration_path = app_path.parent.joinpath('migrations')
        if migration_path.exists():
            shutil.rmtree(migration_path)

if __name__ == '__main__':
    if sys.argv[-1] == 'reverse':
        deinit_migration()
    else:
        init_migration()

