import sys
import os
import signal
import json
import inspect
import math
import subprocess
import shutil
from pathlib import Path

from MySQLdb import _mysql
from cryptography.hazmat.primitives import hashes as crypto_hashes
from common.util.python import import_module_string
from media.renew_certs import *

TEST_DB_MIGRATION_ALIAS = 'db_test_migration'

_is_test_migration_found = lambda cfg: cfg.get('alias') == TEST_DB_MIGRATION_ALIAS

class AbstractTestDatabase:
    def start(self, argv):
        assert len(argv) == 2, "arguments must include (1) app config file (2) liquibase config file"
        setting_path   = argv[0]
        liquibase_path = argv[1]
        f = None
        renew_required = []
        cfg_root = {}
        with open(setting_path, 'r') as f:
            cfg_root = json.load(f)
            test_cfg = list(filter(_is_test_migration_found, cfg_root['databases']))
            if any(test_cfg):
                test_cfg = test_cfg[0]
                test_cfg['liquibase_path'] = liquibase_path
                credential = self.load_db_credential(filepath=test_cfg['credential']['filepath'],
                        hierarchy=test_cfg['credential']['hierarchy'])
                test_cfg['credential'] = credential
                self.setup_test_db(cfg=test_cfg)
            else:
                err_msg = 'the alias `%s` must be present in database configuration file' \
                        % TEST_DB_MIGRATION_ALIAS
                raise ValueError(err_msg)

    def load_db_credential(self, filepath:str, hierarchy):
        target = None
        with open(filepath , 'r') as f:
            target = json.load(f)
            for token in hierarchy:
                target = target[token]
        if target:
            target = {'host' : target['HOST'],  'port' : int(target['PORT']),
                'user' : target['USER'],  'passwd' : target['PASSWORD'] }
        return target

    def setup_test_db(self, cfg):
        raise NotImplementedError()

    def _create_drop_db(self, cfg, sql):
        credential = cfg['credential']
        credential.update({'connect_timeout':30})
        db_conn = None
        try:
            db_conn = _mysql.connect(**credential)
            db_conn.query(sql)
        finally:
            if db_conn:
                db_conn.close()

    def db_schema_cmd(self, cfg):
        credential = cfg['credential']
        return ['%s/liquibase' % cfg['liquibase_path'],
                '--defaults-file=./media/liquibase.properties',
                '--changeLogFile=./media/migration/changelog_media.xml',
                '--url=jdbc:mariadb://%s:%s/%s'
                    % (credential['host'], credential['port'], cfg['db_name']),
                '--username=%s' % credential['user'],
                '--password=%s' % credential['passwd'],
                '--log-level=info']
## end of AbstractTestDatabase


class StartTestDatabase(AbstractTestDatabase):
    def db_schema_cmd(self, cfg):
        out = super().db_schema_cmd(cfg)
        out.append('update')
        return out

    def setup_test_db(self, cfg):
        sql = 'CREATE DATABASE `%s` DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_bin;' \
                % cfg['db_name']
        self._create_drop_db(cfg, sql)
        subprocess.run(self.db_schema_cmd(cfg))


class EndTestDatabase(AbstractTestDatabase):
    def db_schema_cmd(self, cfg):
        out = super().db_schema_cmd(cfg)
        out.extend(['rollback', '0.0.0'])
        return out

    def setup_test_db(self, cfg):
        subprocess.run(self.db_schema_cmd(cfg))
        sql = 'DROP DATABASE `%s`;' % cfg['db_name']
        self._create_drop_db(cfg, sql)


class KillRPCconsumer:
    def start(self, argv):
        assert len(argv) == 1, "arguments must include (1) app config file"
        setting_path  = argv[0]
        pid = -1
        with open(setting_path, 'r') as f:
            cfg_root = json.load(f)
            pid_file = cfg_root['pid_file']['rpc_consumer']
            with open(pid_file, 'r') as f2:
                pid = int(f2.readline())
        if pid > 2:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError  as e:
                print('failed to kill RPC consumer, PID {_pid} not found'.format(_pid=pid))


class FileChunkBasePreprocessor:
    def start_processing(self, fchunk_cfg):
        raise NotImplementedError()

    def start(self, argv):
        assert len(argv) == 1, "arguments must include (1) app config file"
        setting_path  = argv[0]
        fchunk_cfg = None
        with open(setting_path, 'r') as f:
            cfg_root = json.load(f)
            fchunk_cfg = cfg_root['test']['file_chunk']
        if fchunk_cfg and fchunk_cfg.get('output_metadata') and fchunk_cfg.get('files'):
            self.start_processing(fchunk_cfg)
        else:
            print('missing field in config file {cfgpath}'.format(cfgpath=setting_path))


class FilechunkSetup(FileChunkBasePreprocessor):
    def start_processing(self, fchunk_cfg):
        errors = []
        base_path = Path(fchunk_cfg['base_folder'])
        if base_path.is_dir() and os.access(base_path, os.F_OK):
            if not os.access(base_path, os.R_OK | os.W_OK | os.X_OK):
                errmsg = "permission denied error on metadata path {0}".format(metadata_folderpath)
                errors.append(OSError(errmsg))
        else:
            try:
                os.makedirs(base_path, mode=0o777, exist_ok=True)
            except (FileNotFoundError, IOError, OSError)  as e:
                print('error happened on mkdir {0}, detail = {1}'.format(base_path, e))
                errors.append(e)
        if any(errors):
            return
        metadata_filepath = base_path.joinpath(fchunk_cfg['output_metadata'])
        metadata_f = open(metadata_filepath,'w')
        metadata = []
        for f_cfg in fchunk_cfg['files']:
            src_path    = f_cfg.get('src')
            num_chunks  = f_cfg.get('num_chunks', 0)
            dst_path_patt = f_cfg.get('dst_pattern')
            file_type   = f_cfg.get('type')
            broken      = f_cfg.get('broken', False)
            if not src_path or num_chunks <= 0 or not dst_path_patt:
                errmsg = 'incomplete configuration, src_path={0}, num_chunks={1}, dst_path_patt={2}'.format(
                       src_path, num_chunks, dst_path_patt)
                print(errmsg)
                errors.append(IOError(errmsg))
                break
            self._build_filechunks(src_path, base_path=base_path, metadata=metadata, num_chunks=num_chunks,
                    dst_path_patt=dst_path_patt, broken=broken, file_type=file_type)
        json.dump(metadata, fp=metadata_f)
        metadata_f.close()
        ##import pdb
        ##pdb.set_trace()

    def _build_filechunks(self, src_path:str, base_path, num_chunks:int, dst_path_patt:str,
            metadata:list, file_type:str, broken:bool=False):
        page_sz = 2048
        src_f = None
        dst_f = None
        chunk_cfg_list = []
        metadata_item  = {'type':file_type, 'broken':broken, 'chunks':chunk_cfg_list}
        try:
            src_path = Path(src_path)
            tot_sz = Path.stat(src_path).st_size
            chunk_sz_avg = math.ceil(tot_sz / num_chunks)
            src_f = open(src_path ,'rb')
            for idx in range(num_chunks):
                dst_relative_path = dst_path_patt.format(idx)
                dst_file_path = base_path.joinpath(dst_relative_path)
                os.makedirs(dst_file_path.parent, mode=0o777, exist_ok=True)
                dst_f = open(dst_file_path ,'wb')
                digest = crypto_hashes.Hash( crypto_hashes.SHA1() )
                nb_read = 0
                while nb_read < chunk_sz_avg:
                    rd_sz = min(page_sz, chunk_sz_avg - nb_read)
                    readbytes = src_f.read(rd_sz)
                    if readbytes :
                        digest.update(readbytes)
                        dst_f.write(readbytes)
                        nb_read += len(readbytes)
                    else:
                        break
                dst_f.close()
                chunk_cfg_item = {'part':idx + 1, 'checksum':digest.finalize().hex(),
                        'path':str(dst_file_path) }
                chunk_cfg_list.append(chunk_cfg_item)
            metadata.append(metadata_item)
        except (FileNotFoundError, IOError, OSError)  as e:
            print('I/O error on file {0}, detail = {1}'.format(src_path, e))
            errors.append(e)
        except Exception as e2:
            print('Error on file {0}, detail = {1}'.format(src_path, e))
            errors.append(e)
        finally:
            if src_f and not src_f.closed:
                src_f.close()
            if dst_f and not dst_f.closed:
                dst_f.close()

class FilechunkTeardown(FileChunkBasePreprocessor):
    def start_processing(self, fchunk_cfg):
        base_path = Path(fchunk_cfg['base_folder'])
        if base_path.is_dir() and os.access(base_path, os.F_OK):
            shutil.rmtree(base_path)


if __name__ == '__main__':
    curr_module = sys.modules[__name__]
    cls_members = inspect.getmembers(curr_module, inspect.isclass)
    target_class_name = sys.argv[1]
    target_class = None
    for name, cls in cls_members:
        if target_class_name == name:
            target_class = cls
            break
    assert target_class, 'invalid class name "%s" received \n' % target_class_name
    argv = sys.argv[2:]
    target_class().start(argv)

