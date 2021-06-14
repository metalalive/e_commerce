
export const EMPTY_VALUES = [undefined, null, ''];

export const BaseUrl = {
    WEB_HOST: 'http://localhost:8006',
    API_HOST: 'http://localhost:8007',
    USERMGT_HOST   : 'http://localhost:8008',
    PRODUCTMGT_HOST: 'http://localhost:8009',
    // in this project file-upload server host is as the same as image server
    // , if your applition requires to handle high traffic caused by uploading files
    //  and downloading images in short time period, it is better to seperate them
    //  into different servers.
    FILEUPLOAD_HOST: 'http://localhost:8010',
};

const _base_api_header  = {'content-type':'application/json', 'accept':'application/json, text/html'};
const _base_api_req_opt = {mode:"cors", credentials:"include", headers:_base_api_header};

var _api_req_opts_get  = () => ({method:"GET" , ..._base_api_req_opt});
var _api_req_opts_post = () => ({method:"POST", ..._base_api_req_opt});
var _api_req_opts_put  = () => ({method:"PUT" , ..._base_api_req_opt});
var _api_req_opts_delete = () => ({method:"DELETE" , ..._base_api_req_opt});
var _api_req_opts_patch  = () => ({method:"PATCH"  , ..._base_api_req_opt});

export const DEFAULT_API_REQUEST_OPTIONS = {
    GET : _api_req_opts_get,
    POST: _api_req_opts_post,
    PUT : _api_req_opts_put,
    DELETE : _api_req_opts_delete,
    PATCH  : _api_req_opts_patch,
};

