import os

def main():
    from migrations.django  import auto_deploy, render_fixture, _render_usermgt_fixture
    from django.contrib import contenttypes, auth
    import user_management
    apps = (contenttypes, auth, user_management)
    auto_deploy(apps)
    from django import setup
    os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'user_management.settings')
    setup()
    from django.core.management import call_command
    options = {'database':'site_dba',}
    # --- schema migration ---
    call_command('migrate', 'contenttypes', '0002', **options)
    call_command('migrate', 'auth', '0001', **options)
    call_command('migrate', 'user_management', '0001', **options)
    # --- data migration ---
    renderred_fixture_path = render_fixture(src_filepath='user_management/fixtures.json',
            detail_fn=_render_usermgt_fixture)
    options = {'database':'usermgt_service',}
    call_command('loaddata', renderred_fixture_path, **options)
    os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'product.settings')

if __name__ == '__main__':
    main()
