import sys
import os
import json
import inspect
import math
import subprocess
import shutil
from pathlib import Path

from cryptography.hazmat.primitives import hashes as crypto_hashes
from media.renew_certs import *
from media.render_template import *

CFG_DB_MIGRATE_ALIAS = 'app_db_migration'

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
        errmsg_pattern = 'incomplete configuration, src_path={0}, num_chunks={1}, dst_path_patt={2}, file_type={3}, file_subtype={4}'
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
            file_subtype = f_cfg.get('subtype')
            broken      = f_cfg.get('broken', False)
            if not src_path or num_chunks <= 0 or not dst_path_patt or not file_type or not file_subtype:
                errmsg = errmsg_pattern.format(src_path, num_chunks, dst_path_patt, file_type, file_subtype)
                print(errmsg)
                errors.append(IOError(errmsg))
                break
            self._build_filechunks(src_path, base_path=base_path, metadata=metadata, num_chunks=num_chunks,
                    dst_path_patt=dst_path_patt, broken=broken, file_type=file_type, file_subtype=file_subtype)
        json.dump(metadata, fp=metadata_f)
        metadata_f.close()
        ##import pdb
        ##pdb.set_trace()

    def _build_filechunks(self, src_path:str, base_path, num_chunks:int, dst_path_patt:str,
            metadata:list, file_type:str, file_subtype:str, broken:bool=False):
        page_sz = 2048
        src_f = None
        dst_f = None
        chunk_cfg_list = []
        metadata_item  = {'type':file_type, 'subtype':file_subtype, 'broken':broken, 'chunks':chunk_cfg_list}
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

