#### Architecture
- Create an extra layer (data access layer ?) seperate from application logic, in order to easily switch between different databases.
  - currently this application interacts with database by directly running raw SQL
  - the code responsible for producing raw SQL statements spread out across many sections of the codebase.
  - only MariaDB is tested
- Parallel transcoding process after stream metadata is parsed from media container
  - Before decoding, distribute input packets to different transcoding nodes (RPC workers)
  - After encoding, collect encoded packets to certain RPC worker, assemble them to final file(s)
  - only few types of containers can achieve thhs, (e.g. mp4)
- write other less important metadata of transcoded file to database
  - e.g. checksum of each generated file, video title, tags for search, ...etc.
  - such data is rarely changed once inserted
  - document-oriented databases is a good option
- For better scalability of progress monitor, use distributed document-oriented database to keep transcoding progress data.
  - such information is rarely updated once inserted to database / file
  - currently it is maintained in file of a single app server for individual user.
  - that implicitly means you need to connect to the same app server for the progress

#### Implementation
- the code producing raw SQL statements contains **declaration of [prepared statement](https://stackoverflow.com/a/4614159/9853105)**
  - typically prepared statement should be declared **only once** after database connection is successfully established.
  - some functions declare existing prepared statments over and over again, which would slow down application
  - to improve the workflow, create all necessary prepare statements only once each time  after database connection is done.
- sticky mechanism at reverse proxy, for fetching video segments cached in app server
  - 2 API endpoints related to this : `/file/stream/init` and `/file/stream/seek`
  - once app server responds `/file/stream/init` with valid document ID, reverse proxy should add sticky session to the response
  - then client sends file request with the sticky data and document ID to the endpoint `/file/stream/seek`, reverse proxy should be able to know which upstream server the request should be forwarded to.
  - this requires study of Nginx module development
- Potential memory leak with ffmpeg
  - happened after decoding / filtering / encoding over 10k video frames
  - Valgrind always crashes in such case
  - review transcoding process
- Integration test should synchronize max number of database connections with actual database configuration.
- [Rabbitmq/C](https://github.com/rabbitmq/rabbitmq-c) is currently applied to this app for interacting with RabbitMQ, the library does not support asynchronous operations (except consume function)
  - looking for other C libraries which send AMQP requests in non-blocking manner.
- [libh2o](https://github.com/h2o/h2o) might report assertion failure on `h2o_http2_stream_t -> _data.size` in rare unknown cases, the value may be junk (uninitialized) data, figure out how did that happen .

#### Development
- Upgrade Valgrind to latest version then check the memory usage again
  - current version (3.110.1) reports tons of false positive issues (e.g. read/write of unninitialized value)
  - and might cause undefined program behaviour and stack buffer overflow.

