
### Build
#### Schema migration ([liquibase](https://github.com/liquibase/liquibase))
* upgrade
```
cd /PATH/TO/PROJECT_HOME/staff_portal

/PATH/TO/liquibase  --defaults-file=./media/liquibase.properties \
  --changeLogFile=./media/migration/changelog_media.xml  \
  --url=jdbc:mariadb://localhost:3306/ecommerce_media \
  --username=YOUR_DBA_USERNAME  --password=YOUR_DBA_PASSWORD \
  --log-level=info  update

/PATH/TO/liquibase  --defaults-file=./media/liquibase.properties \
  --changeLogFile=./media/migration/changelog_usermgt.xml  \
  --url=jdbc:mariadb://localhost:3306/ecommerce_usermgt \
  --username=YOUR_DBA_USERNAME  --password=YOUR_DBA_PASSWORD \
  --log-level=info  update
```

* downgrade to the state before all tables were initially created
```
/PATH/TO/liquibase  --defaults-file=./media/liquibase.properties \
    --changeLogFile=./media/migration/changelog_media.xml \
    --url=jdbc:mariadb://localhost:3306/ecommerce_media \
    --username=YOUR_DBA_USERNAME  --password=YOUR_DBA_PASSWORD \
    --log-level=info  rollback  0.0.0

/PATH/TO/liquibase  --defaults-file=./media/liquibase.properties \
    --changeLogFile=./media/migration/changelog_usermgt.xml \
    --url=jdbc:mariadb://localhost:3306/ecommerce_usermgt \
    --username=YOUR_DBA_USERNAME  --password=YOUR_DBA_PASSWORD \
    --log-level=info  rollback  0.0.0
```

#### application server
##### Prerequisite
Build system
* [Cmake](https://cmake.org/cmake/help/latest/index.html) >= 3.5.0
* [gcc](https://gcc.gnu.org/onlinedocs/) >= 10.3.0, with [c17](https://en.wikipedia.org/wiki/C17_(C_standard_revision)) stardard

Library Dependencies (for application)
* [H2O](https://github.com/h2o/h2o) >= 2.3.0-DEV
* [brotli](https://github.com/google/brotli)
* [jansson](https://github.com/akheron/jansson) >= 2.14

Library Dependencies (for testing)
* [cgreen](https://github.com/cgreen-devs/cgreen) >= 2.14
* [libcurl](https://github.com/curl/curl) >= 7.69.1
* [nghttp2](https://github.com/nghttp2/nghttp2) >= 1.46.0 , for enabling http/2 in `libcurl`


#### certificate renewal
##### development server
```
python3 -m media.renew_certs media.renew_certs.DevCertRenewal  ./media/settings/development.json
```
##### testing server (for integration test)
```
python3 -m media.renew_certs media.renew_certs.TestCertRenewal  ./media/settings/test.json
```

#### workflow
Generate build files (e.g. makefiles)
```
CC="/PATH/TO/gcc/10.3.0/installed/bin/gcc" \
    PKG_CONFIG_PATH="<YOUR_PATH_TO_PKG_CFG>" \
    cmake -DCMAKE_PREFIX_PATH="/PATH/TO/cgreen/installed"  ..
```
where `<YOUR_PATH_TO_PKG_CFG>` should be :
* `/PATH/TO/brotli/pkgconfig`
* `/PATH/TO/libuv/pkgconfig`
* `/PATH/TO/h2o/pkgconfig`
* `/PATH/TO/jansson/pkgconfig`
* `/PATH/TO/libcurl/pkgconfig`
* `/PATH/TO/nghttp2/pkgconfig`

For those libraries that are NOT integrated with `pkg-config` , add path to `CMAKE_PREFIX_PATH`

after cmake completed successfully, generate executable app server by :
```
make app.out
```

### Run
#### start development server
```
./media/build/app.out  ./media/settings/development.json
```
or 
```
make dev_server
```

To test the development server, you can use web browsers or command-line tools like `cURL`
```
LD_LIBRARY_PATH="/PATH/TO/curl/installed/lib:$LD_LIBRARY_PATH" /PATH/TO/curl --cacert /PATH/TO/ca.crt \
   --key /PATH/TO/ca.private.key  --request GET --http2 --header "Content-Type: application/json" \
   --header "Accept: application/json"  -v  https://localhost:8010/ANY/VALID/PATH
```

#### Run test
##### unit test
```
make unit_test
```
##### Integration test
```
make integration_test
```


NOTE
* the database credential `YOUR_DBA_USERNAME` / `YOUR_DBA_PASSWORD` should have access to perform DDL to the specified database `ecommerce_media` and `ecommerce_usermgt` 

