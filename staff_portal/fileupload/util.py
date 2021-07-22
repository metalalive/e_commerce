import math
import copy
import random
import hashlib
import tempfile
import asyncio

import magic
import av
from PIL import Image

from common.util.python import import_module_string
from .constants import ACCEPT_DUPLICATE, ANONYMOUS_USER, app_cfg

storage_cfg = app_cfg['storage']

def construct_resource_id(hashed:bytes, duplicate:int):
    if ACCEPT_DUPLICATE:
        resource_id = '%s%s' % (hashed.hex(), hex(duplicate)[2:])
    else:
        resource_id = hashed.hex()
    return resource_id


def extract_resource_id(resource_id:str, hash_alg:str='sha1'):
    hashobj = getattr(hashlib, hash_alg)()
    hash_hex_sz = hashobj.digest_size << 1
    hashed = bytes.fromhex(resource_id[:hash_hex_sz])
    if ACCEPT_DUPLICATE:
        duplicate = int(resource_id[hash_hex_sz:], 16)
    else:
        assert len(resource_id) == hash_hex_sz, 'recieved invalid resource_id = %s' % (resource_id)
        duplicate = 0
    return hashed, duplicate


def _determine_filestore_path(hashed:bytes, duplicate:int, postfix:str=''):
    hash_hex_list = map(lambda b: '{0:0{1}x}'.format(b,2), hashed)
    _path = '/'.join(hash_hex_list)
    if postfix:
        _path = '%s_%s_%s.file' % (_path, duplicate, postfix)
    else:
        _path = '%s_%s.file' % (_path, duplicate)
    return _path


def  get_storage():
    try:
        FileSystemStorage
    except NameError:
        from common.util.python.storage import FileSystemStorage
    finally:
        out = FileSystemStorage(location=storage_cfg['base_location'])
    return out


def retrieve_file_type(_file, hdr_sz=64):
    if asyncio.iscoroutinefunction(_file.seek):
        asyncio.run(_file.seek(0))
    else:
        _file.seek(0)
    if asyncio.iscoroutinefunction(_file.read):
        header_chunk = asyncio.run(_file.read(hdr_sz)) # in case it is big file
    else:
        header_chunk = _file.read(hdr_sz)
    return magic.from_buffer(header_chunk, mime=True)


class FileAttributeSetFactory:
    mimetype_map = app_cfg['valid_mime_types']

    def __new__(cls, media_type=None, **kwargs):
        mime_key = '/'.join(media_type) if media_type else ''
        module_path = cls.mimetype_map.get(mime_key, 'fileupload.util.BaseFileAttributeSet')
        module = import_module_string(module_path)
        if not issubclass(module, BaseFileAttributeSet):
            raise TypeError('incorrect module type: %s' % module_path)
        return module(media_type=media_type, **kwargs)


class BaseFileAttributeSet:
    """ internal class for generating attributes of a file """
    def __init__(self, hash_:bytes=None, duplicate:int=0, owner=None, media_type=None,
            acl:dict=None, _file=None):
        self._hash = hash_
        self._duplicate = duplicate
        self._owner = owner
        self._mimetype = media_type
        acl = copy.copy(acl) if acl else {} # shallow copy any item in acl
        for prof_id, ac in acl.items():
            acl[prof_id] = UserFileAccessControl(**ac)
        self._acl_usrs = acl
        self._acl_default = UserFileAccessControl()
        self._storage = get_storage()
        self._file = _file
        if _file:
            self._file.mimetype = media_type

    @property
    def owner(self):
        return self._owner

    @property
    def mimetype(self):
        return self._mimetype

    def access_control(self, prof_id=ANONYMOUS_USER):
        return  self._acl_usrs.get(prof_id, self._acl_default)

    def fetch_file(self, manual_close=False, **extra_attrs):
        """
        The instance of this class will NOT close all the files opened by this function,
        application callers are responsible to close all the opened files.
        """
        # also it is not a good practice to close all the opened files on
        # descructing the instance of this class
        if self._file:
            return self._file
        if not self._hash:
            raise FileNotFoundError()
        extra_attrs = dict(filter(lambda kv:kv[1] is not None, extra_attrs.items()))
        filename_postfix = self.get_filename_postfix(**extra_attrs)
        filepath = _determine_filestore_path(hashed=self._hash ,
                duplicate=self._duplicate, postfix=filename_postfix)
        _file = None
        try:
            _file = self._storage.open(filepath, manual_close=manual_close)
        except FileNotFoundError as e:
            _file = self.handle_file_not_found(filepath=filepath, **extra_attrs)
            if _file is None:
                raise
        if _file:
            _file.mimetype = retrieve_file_type(_file, hdr_sz=74).split('/')
        self._file = _file
        return _file

    def get_filename_postfix(self, **extra_attrs):
        pass

    def handle_file_not_found(self, filepath, **extra_attrs):
        pass

    def upload_post_process(self, **kwargs):
        # subclasses may return attributes declared or auto-configured
        # during the post process.
        kwargs = dict(filter(lambda kv:kv[1] is not None, kwargs.items()))
        return self._upload_post_process(**kwargs)

    def _upload_post_process(self, **kwargs):
        # subclasses should overwrite this function
        pass
## end of class BaseFileAttributeSet


class UserFileAccessControl:
    def __init__(self, rd:bool = False, rd_acl:bool = False):
        self._rd = rd
        self._rd_acl = rd_acl

    @property
    def can_read(self):
        return self._rd

    @property
    def can_read_acl(self):
        return self._rd_acl


class ImageFileAttributeSet(BaseFileAttributeSet):
    # In this project , I generate image thumbnail and video screenshot on the fly
    # to fit different needs of frontend application. If the thumbnail shape is
    # predefined, I can also generate thumbnail on file upload
    _app_cfg = app_cfg['post_process']['thumbnail']
    def _upload_post_process(self, **kwargs):
        return {'thumbnail':{'default_shape':{'width':self._app_cfg['width'],
                'height':self._app_cfg['height']}, }}

    def get_filename_postfix(self, width:int=0, height:int=0, **kwargs):
        if width > 0 and height > 0:
            return 'thumbnail_%s_%s' % (height,width)

    def handle_file_not_found(self, filepath, width:int=0, height:int=0, **extra_attrs):
        # if it is the path to thumbnail, re-generate it,  otherwise, raise error instead
        origin_filepath = _determine_filestore_path(hashed=self._hash, duplicate=self._duplicate)
        if origin_filepath == filepath:
            return
        origin_file = None
        origin_img = None
        processed_file = None
        try: # load the entire file synchronously, then generate thumbnail 
            origin_file = self._storage.open(origin_filepath) # TODO, how to handle huge file ?
            origin_img    = Image.open(origin_file)
            thumbnail_img = gen_img_thumbnail(src=origin_img, new_height=height,
                    new_width=width, pos_y=0, pos_x=0)
            processed_file = tempfile.SpooledTemporaryFile(max_size=2048)
            thumbnail_img.save(processed_file, format=self.mimetype[1])
            processed_file.seek(0)
            self._storage.save(path=filepath, content=processed_file, async_flg=False,
                    alt_path_autogen=False, chunk_sz=storage_cfg['chunk_size_bytes'])
            processed_file.seek(0)
        except Exception as e:
            if processed_file:
                processed_file.close()
                processed_file = None
            raise # error might be related to missing original image
        finally:
            if origin_img:
                origin_img.close()
                origin_img = None
            if origin_file:
                origin_file.close()
                origin_file = None
        return  processed_file
## end of class ImageFileAttributeSet


class VideoFileAttributeSet(BaseFileAttributeSet):
    def get_filename_postfix(self, width:int=0, height:int=0, **kwargs):
        if width > 0 and height > 0:
            return  'snapshot_%s_%s' % (height,width)

    def _choose_stream(self, container, stream_idx):
        try:
            return container.streams.video[stream_idx]
        except av.FFmpegError as ffme: # stream may corrupt
            raise IOError('video stream %s corrupted' % stream_idx)

    def _gen_snapshot(self, _file, vid_stream_idx:int=0, snapshot_at:float=-1.0):
        snapshot = None
        with av.open(_file) as container:
            try:
                chosen_stream = self._choose_stream(container, vid_stream_idx)
            except IndexError as ie:
                vid_stream_idx = 0
                chosen_stream = self._choose_stream(container, 0)
            # `snapshot_at` given from frontend request indicates the point of time
            # at which backend decoder takes a screenshot in seconds.
            seek_by_pts = (container.format.flags & av.format.Flags.SEEK_TO_PTS) == av.format.Flags.SEEK_TO_PTS
            if seek_by_pts:
                if snapshot_at >= 0: # convert to seek time in unit of `time_base`
                    seek_time = int(1.0 * snapshot_at / chosen_stream.time_base)
                else:# randomly choose any frame as a snapshot
                    seek_time = random.randrange(chosen_stream.duration)
                    snapshot_at = 1.0 * seek_time * chosen_stream.time_base
            else: # TODO, log error and fgure out how to handle the error
                raise ValueError('unknown method to seeking frames in video')
            # any_frame is False, which means the decoder will start from
            # nearest keyframe, which is essential for snapshot
            container.seek(seek_time, stream=chosen_stream, any_frame=False)
            for frame in container.decode(video=vid_stream_idx):
                if seek_by_pts:
                    now_time = frame.pts
                else:
                    break
                if now_time > seek_time:
                    snapshot = frame.to_image()
                    break
        return snapshot, snapshot_at


    _app_cfg = app_cfg['post_process']['screenshot']

    def _upload_post_process(self, width:int=_app_cfg['width'], height:int=_app_cfg['height'],
            vid_stream_idx:int=0, snapshot_at:float=-1.0, **kwargs):
        # check whether the file object supports async operations
        _file = self.fetch_file(manual_close=False)
        snapshot, snapshot_at = self._gen_snapshot(_file, vid_stream_idx, snapshot_at)
        if not snapshot: # TODO : log error and return
            return
        processed_file = tempfile.SpooledTemporaryFile(max_size=2048)
        try: # generate thumbnail and save it to storage
            thumbnail_img = gen_img_thumbnail(src=snapshot, new_height=height, new_width=width)
            thumbnail_img.save(processed_file, format='jpeg')
            filename_postfix = self.get_filename_postfix(width, height)
            filepath = _determine_filestore_path(hashed=self._hash ,
                    duplicate=self._duplicate, postfix=filename_postfix)
            processed_file.seek(0)
            self._storage.save(path=filepath, content=processed_file, async_flg=False,
                    alt_path_autogen=False, chunk_sz=storage_cfg['chunk_size_bytes'])
        finally:
            processed_file.close()
        return {'snapshot':{'shape':{'width': width, 'height':height}, 'time_at':snapshot_at,
                 }, 'stream_index':vid_stream_idx}
    ## end of upload_post_process()
## end of class VideoFileAttributeSet


def gen_img_thumbnail(src, new_height:int, new_width:int, pos_y:int=0, pos_x:int=0):
    def assert_img_shape(name:str, value:int):
        assert value > 0, '%s has to be positive integer, but receives %s' % (name,value)
    def assert_img_pos(name:str, value:int):
        assert value >= 0, '%s has to be non-negative integer, but receives %s' % (name,value)
    assert_img_shape('new_height', new_height)
    assert_img_shape('new_width', new_width)
    assert_img_pos('pos_y', pos_y)
    assert_img_pos('pos_x', pos_x)
    # `src` is a PIL Image instance
    old_width, old_height  = src.size
    assert old_height >= new_height, 'To generate image thumbnail, new height (%s) \
             has to be less than original height (%s)' % (new_height, old_height)
    assert old_width >= new_width  , 'To generate image thumbnail, new width (%s) \
            has to be less than original width (%s)' % (new_width, old_width)
    old_ratio = old_height / old_width
    new_ratio = new_height / new_width
    if old_ratio == new_ratio:
        cropped = src
    else:
        if old_height < old_width:
            # new_height < new_width --> new_ratio < 1, means the width can be extended
            crop_width  = old_height / new_ratio
            crop_height = old_height
            if crop_width > old_width: # limit longer boundary
                truncate_ratio = crop_width / old_width
                crop_width  = old_width
                crop_height = crop_height / truncate_ratio
        else:
            # new_height < new_width --> new_ratio < 1, means the height can be extended
            crop_width  = old_width
            crop_height = old_width / new_ratio
            if crop_height > old_height: # limit longer boundary
                truncate_ratio = crop_height / old_height
                crop_width  = crop_width / truncate_ratio
                crop_height = old_height
        pos_y_2 = pos_y - 1 + math.floor(crop_height)
        pos_x_2 = pos_x - 1 + math.floor(crop_width)
        cropped = src.crop((pos_x, pos_y, pos_x_2, pos_y_2))
    return cropped.resize((new_width, new_height))


