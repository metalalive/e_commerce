
### Requirement

| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.9 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 10.3.22 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB_server_setup.md) |
|RabbitMQ| 3.2.4 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/rabbitmq_setup.md) |
|Elasticsearch| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#elasticsearch) | 
|Logstash| 5.6.16 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/ELK_setup.md#logstash) |
|Kibana| 5.6.16 | N/A |
|virtualenv| 20.0.20 | N/A|
|OpenSSL| 1.1.1 | [see here](https://raspberrypi.stackexchange.com/a/105663/86878) |

and those required python libraries in [requirement.txt](./requirement.txt)
Note
* upgragde PIP to latest version after installing python
* install virtualenv , create python virtual environment only for this backend system.
* install C extension built for this project, by running `python ./common/util/c/setup.py install --record ./tmp/setuptools_install_files.txt` . Once you need to remove the installed extension , run `python -m pip uninstall my-c-extention-lib ; rm -rf ./build`
* switch to the virtual environment you created above, before installing all other required libraries.


