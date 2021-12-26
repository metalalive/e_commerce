
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


NOTE
* the database credential `YOUR_DBA_USERNAME` / `YOUR_DBA_PASSWORD` should have access to perform DDL to the specified database `ecommerce_media` and `ecommerce_usermgt` 

