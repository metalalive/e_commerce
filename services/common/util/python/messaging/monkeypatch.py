
def patch_kombu_pool():
    from kombu.connection import ConnectionPool
    def patched_conn_pool_prepare(self, resource):
        if callable(resource):
            resource = resource()
        resource._debug('acquired')
        resource._acquired_from_pool = True
        return resource

    if not hasattr(ConnectionPool.prepare , '_patched'):
        ConnectionPool.prepare =  patched_conn_pool_prepare
        setattr(ConnectionPool.prepare , '_patched', None)


