from typing   import List, Optional, Union
from datetime import datetime
from tempfile import NamedTemporaryFile
import hashlib
import asyncio
import traceback
# patch this before importing pydantic referenced by fastapi
from common.util.python import monkeypatch_typing_specialform
monkeypatch_typing_specialform()

from pydantic import BaseModel as PydanticComplexType # like C struct ?
from fastapi import APIRouter, UploadFile, File, Header, Depends as FastapiDepends
from fastapi import HTTPException as FastApiHTTPException, status as FastApiHTTPstatus
from fastapi.responses import StreamingResponse, JSONResponse
from fastapi.security  import OAuth2AuthorizationCodeBearer

from common.models.db import EmptyDataRowError
from common.util.python.fastapi.auth import base_permission_check, base_authentication, get_unverified_token_payld

from .constants import app_cfg
from .models import check_upload_quota, save_upload_file, get_file_attrs
from .util   import extract_resource_id, retrieve_file_type, FileAttributeSetFactory

"""
Implementation note of this file-uploading service
* each uploaded file has following states :
    * unconfirmed, the newly uploaded file but no other
      service has claimed to use this file.
    * confirmed, there is at least one service(s) that claimed
      to use this uploaded file.

* valid state transitions:
    * (INIT) --> (unconfirmed)
      each newly uploaded file is initially in this state
    * (unconfirmed) --> (confirmed)
      user claims that other specific service will use this file
    * (confirmed) --> (confirmed)
      It is OK for the same file to be reclaimed several times
    * (confirmed) --> (unconfirmed)
      Once a file is no longer used by any other service, it
      will switch back to `unconfirmed` state.
    * (unconfirmed) --> (DELETED)
      Delete unconfirmed files by cron job periodically

* APIs are accessible to all users authenticated and authorized
  using JWT access token authentication

* this service handles access control of each file, and relationship
  with all other services which reference to each file at here.
"""

def monkeypatch_starlette_uploadfile():
    """
    starlette.formparsers.MultiPartParser.parse() directly creates instance of
    starlette.datastructures.UploadFile  when uploading file , it is NEITHER
    configurable nor changable by any other implementation (e.g. by subclassing
    MultiPartParser) , so you will have trouble when you attempt to subclass
    UploadFile , currently the cost-effective way is likely to monkey-patch
    UploadFile because MultiPartParser.parse() contains very complex logic which
    makes it difficult to overwrite and maintain .
    """
    from starlette.datastructures import UploadFile as StarletteUploadFile
    old_init = StarletteUploadFile.__init__
    old_write = StarletteUploadFile.write
    old_seek  = StarletteUploadFile.seek

    def patched_init(self, *args, **kwargs):
        old_init(self, *args, **kwargs)
        # add extra synchronous file operation(s)
        self.tell = self.file.tell
        # [workaround] use one more temporary file for video/audio processing library
        # , because many of the libraries still rely on valid path of input file
        # to do things, so SpooledTemporaryFile does NOT work with such libraries.
        self._named_mirror = NamedTemporaryFile()
        self._file_size_bytes = 0

    def patched_del(self):
        self._named_mirror.close()

    async def patched_write(self, data:Union[str,bytes]):
        #result = await super().write(data=data)
        result = await old_write(self, data=data)
        self._named_mirror.write(data)
        self._file_size_bytes = self.tell()
        return result

    async def patched_seek(self, offset: int):
        #result = await super().seek(offset=offset)
        result = await old_seek(self, offset=offset)
        self._named_mirror.seek(offset)
        return result

    def patched_named_mirror(self):
        return self._named_mirror

    def patched_size(self):
        return self._file_size_bytes

    if not hasattr(StarletteUploadFile.__init__, '_patched'):
        StarletteUploadFile.__init__ = patched_init
        StarletteUploadFile.__del__  = patched_del
        StarletteUploadFile.write  = patched_write
        StarletteUploadFile.seek   = patched_seek
        setattr(StarletteUploadFile, 'named_mirror', property(patched_named_mirror))
        setattr(StarletteUploadFile, 'size', property(patched_size))
        setattr(StarletteUploadFile.__init__, '_patched', True)
## end of monkeypatch_starlette_uploadfile()
monkeypatch_starlette_uploadfile()


router = APIRouter(
            prefix='/file' ,
            tags=['generic_file'] ,
            # TODO: dependencies are function executed before hitting
            # the API endpoint , (as middleware in Django ?)
            dependencies=[],
            responses={
                FastApiHTTPstatus.HTTP_404_NOT_FOUND: {'description':'file not found'},
                FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR: {'description':'internal server error'}
            }
        )

class UploadedFileResponse(PydanticComplexType):
    uploader   : str # user profile ID 
    resource_id : str # indicate resource path under the storage host
    time : float # timestamp
    mimetype : str
    post_process:Optional[dict] = {}
    ##signature: str # signature of the uploaded file, optional verification at app tier
    ##delete_token: str # one-time use token for deleting this file



oauth2_scheme = OAuth2AuthorizationCodeBearer(
        authorizationUrl="no_auth_url",
        tokenUrl="http://localhost:8007/usermgt/remote_auth"
    )

async def common_authentication(token:str=FastapiDepends(oauth2_scheme)):
    audience = ['fileupload']
    error_obj = FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_401_UNAUTHORIZED,
            detail='authentication failure',
            headers={'www-Authenticate': 'Bearer'}
        )
    return base_authentication(token=token, audience=audience, error_obj=error_obj)


# TODO, may be re-used by other API endpoints
async def uploadfile_authorization(user:dict=FastapiDepends(common_authentication)):
    error_obj = FastApiHTTPException(
        status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
        detail='authorization failure',  headers={}
    )
    required_roles = set(['fileupload.upload_files',]) # 'fileupload.XxXxXx'
    user = base_permission_check(user=user, required_roles=required_roles,
            error_obj=error_obj)
    quota = user.get('quota', None)
    prof_id = user.get('prof_id', None)
    if not quota or not prof_id:
        raise error_obj
    limit_mbytes = quota.get('fileupload.uploadedfile', 0)
    if not isinstance(limit_mbytes, int) or limit_mbytes <= 0:
        raise error_obj
    return user


def _uploadfile_quota_check(_file, user):
    limit_mb = user['quota']['fileupload.uploadedfile']
    quota_exceeded =  check_upload_quota( uploader=user['prof_id'],
            limit_mb=limit_mb, newfile_sz_b=_file.size )
    if quota_exceeded:
        raise FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_403_FORBIDDEN,
            detail='upload quota exceeded %s MBytes' % limit_mb,
        )


@router.post('/',)
def upload_file(single_file:UploadFile = File(...),
        user:dict = FastapiDepends(uploadfile_authorization),
        # separate API endpoints will be required if this application needs to support
        # more file types, currently only few file types are supported: picture, video
        # , audio, pdf, so there is no need to separate endpoints for them
        width:Optional[int] = None,
        height:Optional[int] = None,
        vid_stream_idx:Optional[int] = None,
        snapshot_at:Optional[float] = None):
    """
    upload a file from frontend client
    * user should be granted with file upload permission
    * hash the file content for storage location
    * check quota limit of current user and save the file to storage
    * return resource ID that consists of file hash within HTTP response
    """
    # in this project, application server does NOT limit maximum request body
    # such check will be done by web server like Nginx at deployment phase
    _uploadfile_quota_check(_file=single_file, user=user)
    mimetype = _validate_file_type(single_file)
    hashed = asyncio.run(_stat_file(single_file))
    dup_err = FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_400_BAD_REQUEST,
            detail='the uploading file already exists',
        )
    resource_id = save_upload_file(hashed=hashed, prof_id=user['prof_id'], _file=single_file,
            mimetype=mimetype, dup_err=dup_err, non_file_types=[UploadFile])
    # post-processing after storing uploaded file
    postproc_result = _upload_file_post_process(_file=single_file, resource_id=resource_id, mimetype=mimetype,
            width=width, height=height, vid_stream_idx=vid_stream_idx, snapshot_at=snapshot_at)
    # TODO
    # sign the uploaded file using private key , return signature
    # figure out how the client can make use of the signature
    resp_kwargs = {'uploader':user['prof_id'], 'resource_id': resource_id,
            'mimetype':mimetype, 'time': datetime.utcnow().timestamp() }
    if postproc_result:
        resp_kwargs['post_process'] = postproc_result
    return UploadedFileResponse(**resp_kwargs)


def _validate_file_type(_file, valid_mime_types=None, hdr_sz=64):
    mimetype = retrieve_file_type(_file=_file, hdr_sz=hdr_sz)
    valid_mime_types = valid_mime_types or app_cfg['valid_mime_types'].keys()
    is_valid_type = mimetype in valid_mime_types
    if not is_valid_type:
        raise FastApiHTTPException(
            status_code=FastApiHTTPstatus.HTTP_422_UNPROCESSABLE_ENTITY,
            detail='invalid file type, %s not supported' % mimetype,
            headers={'allowed_content_types': ','.join(valid_mime_types)}
        )
    return mimetype


async def _stat_file(_file, alg:str='sha1', nbytes_read:int=256):
    # get statistics information of the uploading file, such as calculate file hash
    content = b''
    hash_fn = getattr(hashlib, alg)
    hash_obj = hash_fn()
    await _file.seek(0)
    while True:
        content = await _file.read(nbytes_read) # in case it is big file
        if content: # TODO, how to configure the read content to be byte (not char)
            hash_obj.update(content)
        else:
            break
    hashed = hash_obj.digest()
    await _file.seek(0)
    return hashed


def _upload_file_post_process(_file, resource_id, mimetype, **postproc_kwargs):
    mimetype = mimetype.split('/')
    valid_filepath_required = ['video']
    if mimetype[0] in valid_filepath_required:
        _file = _file.named_mirror # NamedTemporaryFile
    else:
        _file = _file.file # SpooledTemporaryFile
    try:
        hashed, duplicate = extract_resource_id(resource_id, hash_alg='sha1')
        kwargs = {'hash_':hashed, 'duplicate':duplicate, '_file':_file,
                'media_type':mimetype}
        fileattrs = FileAttributeSetFactory(**kwargs)
        return fileattrs.upload_post_process(**postproc_kwargs)
    except Exception as e:
        # TODO, log error, handle the error for any incomplete post process
        raise


def fetchfile_authorization(token, unverified_prof_id, fileattrs, response):
    usr_can_read      = fileattrs.access_control(str(unverified_prof_id)).can_read
    everyone_can_read = fileattrs.access_control().can_read
    has_access = False

    if everyone_can_read:
        has_access = True
    elif usr_can_read:
        # go back to normal authentication process
        audience = ['fileupload']
        verified = base_authentication(token=token, audience=audience)
        if verified and unverified_prof_id and verified.get('prof_id', None) == unverified_prof_id:
            has_access = True
        else:
            response.status_code = FastApiHTTPstatus.HTTP_401_UNAUTHORIZED
            response.body = response.render('authentication failure')
            response.headers['www-Authenticate'] = 'Bearer'
    else:
        response.status_code =  FastApiHTTPstatus.HTTP_403_FORBIDDEN
        response.body = response.render('the user does not have access to the file')
    return has_access


def _gen_file_outstream(content, chunk_sz:int):
    # start fetching file at here, in order to  make sure that the
    # file object is always in open state.
    content.seek(0)
    while True:
        chunk = content.read(chunk_sz)
        if chunk:
            yield chunk
        else:
            break
    content.close()


# TODO, what if frontend users attempt to declare their own resource_id ?
# (currently all the resource_id is generated automatically by backend code)
@router.get('/{resource_id}', status_code=FastApiHTTPstatus.HTTP_200_OK)
def fetch_file(resource_id:str, response:JSONResponse ,
        token:Optional[str] = FastapiDepends(oauth2_scheme),
        width:Optional[int] = None,
        height:Optional[int] = None):
    """
    the user who is granted the read permission can access the uploaded files
    specified by `resource_id`.
    """
    try:
        unverified_prof_id = get_unverified_token_payld(token).get('prof_id', '')
        fileattrs = get_file_attrs(resource_id, prof_ids=[unverified_prof_id])
        has_access = fetchfile_authorization(token, unverified_prof_id, fileattrs, response)
        if has_access:
            extra_attrs = {'width':width, 'height':height}
            _file = fileattrs.fetch_file(manual_close=True, **extra_attrs)
            mimetype = '/'.join(_file.mimetype)
            iterator = _gen_file_outstream(_file, chunk_sz=1024)
            response = StreamingResponse(iterator, media_type=mimetype)
    except (EmptyDataRowError,FileNotFoundError) as e:
        response.status_code = FastApiHTTPstatus.HTTP_404_NOT_FOUND
        response.body = response.render('file not found')
    except Exception as e: # TODO, log uncaught error
        traceback.print_exc()
        response.status_code = FastApiHTTPstatus.HTTP_500_INTERNAL_SERVER_ERROR
        response.body = response.render('internal server error')
    return response



@router.delete('/{resource_id}', responses={403: {'description':'banned operation'}})
async def discard_file(resource_id:str, token:Optional[str] = ''):
    """
    delete the uploaded file synchronously, the argument `token` comes from
    `delete_token` of file metadata returned by upload_file()
    """
    # may return 404 or 410
    return None



class FileACLGrantee(PydanticComplexType):
    prof_id: str
    permissions: list

class FileACLBody(PydanticComplexType):
    grant: List[FileACLGrantee]


@router.put('/{resource_id}/acl', responses={403: {'description':'banned operation'}})
async def edit_access_control(resource_id:str, body:FileACLBody):
    return None


@router.get('/{resource_id}/acl', responses={403: {'description':'banned operation'}})
async def read_access_control_list(resource_id:str):
    grant = []
    body = FileACLBody(grant=grant)
    return body


