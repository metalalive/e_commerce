from setuptools import setup, Extension

setup(
    # Installed package path can be set by `ext_package` or part
    # of `name` argument in Extension instance (see below) .

    # Installed package path  should be different from the source
    # code path , otherwise the installed package would never be
    # able to import (TODO, do they have to be always different ?)
    #packages=['common.util.c'],
    #ext_package='common_ext.util.c',
    ext_modules=[
        Extension(
            name='c_exts.util.keygen',
            sources=['./keygen.c'],
            define_macros=[
                ## ('OPENSSL_API_COMPAT','2')
            ],
            include_dirs=['/usr/local/include'],
            library_dirs=['/usr/local/lib'],
            libraries=['ssl', 'crypto'],
            runtime_library_dirs=['/usr/local/lib'],
            extra_compile_args=['-Wall', '-g', '-gdwarf-2'],
        ),
    ], # end of ext_modules
) # end of setup()

# python -m pip uninstall my-c-extention-lib ; rm -rf ./build
# python ./common/util/c/setup.py install --record ./tmp/setuptools_install_files.txt
# python -c "from c_exts.util import keygen"

