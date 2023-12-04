import React from 'react';
import {BaseUrl, DEFAULT_API_REQUEST_OPTIONS, IMAGE_THUMBNAIL_SHAPE, VIDEO_SNAPSHOT_SHAPE} from '../js/constants.js';
import {BaseExtensibleForm} from '../js/common/react.js';

let api_base_url = {
    // acquire new access token signed in authentication server
    // , the access token returned by the API can be used in specific
    // resource service (in this case, it can be used in file-upload
    // service)
    remote_auth : {host:BaseUrl.API_HOST, path: '/usermgt/remote_auth'},
    // common API endpoint for uploading all types of files ,
    upload_singular: {host:BaseUrl.FILEUPLOAD_HOST , path: '/file/'},
    // path to uploaded file on file-uploading server. Note the file will be deleted
    // asynchronously if it has never been referenced by any other application.
    fetch_uploaded_file  : {host:BaseUrl.FILEUPLOAD_HOST, path: '/file/{0}'},
    // edit access control (ACL) of the uploaded file
    file_acl : {host:BaseUrl.FILEUPLOAD_HOST, path: '/file/{0}/acl'},
    // image paths stored in product application
    referred_product_image : {host:BaseUrl.API_HOST, path: '/product/saleable_item/{0}/image/{1}/thumbnail'},
};

let _fileupload_access_token = null;


const jwt = require('jwt-simple');

function check_token_expired(token) {
    // TODO, refactor , commonly used function ?
    let expired = true;
    if(token) {
        let decoded = jwt.decode(token, null, true); // verify without secret
        let _exp = decoded.exp;
        if(_exp) {
            let _now = Date.now();
            expired = _exp * 1000 < _now;
        }
    }
    return expired;
}


function _get_query_params_image(data) {
    let shape = {}; // data.thumbnail.default_shape
    shape.width  = IMAGE_THUMBNAIL_SHAPE.width;
    shape.height = IMAGE_THUMBNAIL_SHAPE.height;
    return shape;
}

function _get_query_params_video(data) {
    let shape = {
        width:data.snapshot.shape.width ,
        height:data.snapshot.shape.height
    };
    return shape;
}


export class SaleableItemPicture extends BaseExtensibleForm {
    constructor(props) {
        let _valid_fields_name = ['thumbnail', 'resource_id'];
        super(props, _valid_fields_name);
    }

    _normalize_fn_thumbnail(referrer) {
        // `obj` could be dictionary object which stores old value
        // , or DOM element which stores new value of a field.
        // `thumbnail` flag should be ignored on submit, so this
        //  normalization function always return the same empty value
        return undefined;
    }

    _normalize_fn_resource_id(value) {
        return String(value);
    }

    // _serializer_reducer(key, value) {
    //     let value = BaseExtensibleForm.._serializer_reducer(this, key, value);
    //     if(value) {
    //         var skip_fields = ["thumbnail"];
    //         value = skip_fields.includes(key) ? undefined: value;
    //     }
    //     return value;
    // }

    _get_remote_access_token(props) {
        let url = api_base_url.remote_auth.host + api_base_url.remote_auth.path;
        let req_opt = DEFAULT_API_REQUEST_OPTIONS.POST();
        let callbacks = {succeed:[this._get_remote_token_succeed_callback.bind(this)]};
        req_opt.body = JSON.stringify({audience: ['fileupload']});
        let api_kwargs = {base_url:url, req_opt:req_opt, callbacks:callbacks,
             params:{}, extra:{chosen_files: props.chosen_files},};
        this.api_caller.start(api_kwargs);
    }

    _get_remote_token_succeed_callback(data, res, props) {
        let req_args = res.req_args;
        let _props = {chosen_files: req_args.extra.chosen_files,
              access_token: data.access_token};
        _fileupload_access_token = data.access_token;
        this._upload(_props);
    }

    _upload(props) {
        // TODO, would be better to let frontend generate unique key for each uploading file
        let url = api_base_url.upload_singular.host + api_base_url.upload_singular.path; // str.format(key);
        let req_opt = DEFAULT_API_REQUEST_OPTIONS.POST();
        let callbacks = {succeed:[this._upload_succeed_callback.bind(this)]};
        let _form_data = new FormData();
        let query_params = {};
        // current frontend support only one file upload at a time
        let chosen_file = props.chosen_files[0];
        _form_data.append("single_file", chosen_file);
        if(chosen_file.type.split("/")[0] == "video") {
            query_params = {... VIDEO_SNAPSHOT_SHAPE};
        }
        delete req_opt.headers['content-type']; // no need to set `multipart/form-data`
        req_opt.headers['Authorization'] = ("Bearer {0}").format(props.access_token);
        req_opt.body = _form_data;
        let api_kwargs = {base_url:url, req_opt:req_opt, callbacks:callbacks,
             params:query_params, extra:{},};
        this.api_caller.start(api_kwargs);
    }

    _upload_succeed_callback(data, res, props) {
        console.log("_upload_succeed_callback, data: "+ data);
        let mimetype = data.mimetype;
        let filetype_map = {
            application: {api:api_base_url.fetch_uploaded_file, get_query_params:null},
            image : {api:api_base_url.fetch_uploaded_file, get_query_params:_get_query_params_image,
                     fetchfile_succeed_callback:this._fetchfile_succeed_callback_image.bind(this) },
            video : {api:api_base_url.fetch_uploaded_file, get_query_params:_get_query_params_video,
                     fetchfile_succeed_callback:this._fetchfile_succeed_callback_image.bind(this) },
        };
        // immediately load thumbnail or after upload succeeded
        let kv_filetype_map = Object.entries(filetype_map);
        let chosen_filetype_map = kv_filetype_map.filter(entry => mimetype.startsWith(entry[0]));
        if(chosen_filetype_map.length == 1) {
            let entry    = chosen_filetype_map[0];
            let map_item = entry[1];
            if(!map_item.get_query_params) {
                return ;
            }  // no need to load the file
            let url = map_item.api.host + map_item.api.path.format(data.resource_id);
            let query_params = map_item.get_query_params(data.post_process);
            let req_opt = DEFAULT_API_REQUEST_OPTIONS.GET();
            req_opt.headers['Authorization'] = res.req_args.req_opt.headers.Authorization ;
            let callbacks = {succeed:[map_item.fetchfile_succeed_callback]};
            let api_kwargs = {base_url:url, req_opt:req_opt, callbacks:callbacks,
                 params:query_params, extra:{},};
            this.api_caller.start(api_kwargs);
        } else {
            throw "invalid response from file uploaded";
        }
    } // end of _upload_succeed_callback()


    _fetchfile_succeed_callback_image(data, res, props) {
        // the data is Blob instance now, translate it to local URL then
        // re-render this react component
        let local_url = URL.createObjectURL(data);
        let resource_id = res.req_args.base_url.split('/').pop();
        let val = {thumbnail: local_url , resource_id:resource_id};
        this.new_item(val, true);
    }

    _evt_upload_btn_click(e) {
        let props = {chosen_files: e.target.files};
        if(check_token_expired(_fileupload_access_token)) {
            this._get_remote_access_token(props)
        } else {
            props.access_token = _fileupload_access_token;
            this._upload(props);
        }
    }

    _prompt_file_dialog(evt){
        let file_elm = document.createElement('input');
        file_elm.type = "file";
        file_elm.onchange = this._evt_upload_btn_click.bind(this);
        file_elm.click();
    }

    _discard_files(evt) {
        let clicked = this.state.saved.filter((item) => {
            let chkbox_ref = item.refs.resource_id.current;
            return chkbox_ref.checked;
        });
        clicked.map((val) => {
            this.remove_item(val, true);
        });
    }

    _single_item_render(val, idx) {
        return (
            <div className="col-6 col-sm-4">
                <label className="form-imagecheck mb-2">
                    <input name="form-imagecheck" type="checkbox"  ref={val.refs.resource_id}
                        defaultValue={val.resource_id} className="form-imagecheck-input" />
                    <span className="form-imagecheck-figure">
                        <img src={val.thumbnail} ref={val.refs.thumbnail} alt="product image"
                            className="form-imagecheck-image" />
                    </span>
                </label>
            </div>);
    } // end of _single_item_render()

    render() {
        let uploaded_images = this.state.saved.map(this._single_item_render_wrapper.bind(this));
        return (
                <div className="mb-3">
                    <button className="btn btn-primary btn-pill ml-auto" onClick={this._discard_files.bind(this)}>
                        Discard
                    </button>
                    <div className="row row-sm">
                        {uploaded_images}
                        <div className="col-6 col-sm-4">
                            <label className="form-imagecheck mb-2">
                                <button title="upload" className="btn btn-info btn-pill ml-auto" onClick={this._prompt_file_dialog.bind(this)}>
                                    <svg xmlns="http://www.w3.org/2000/svg" className="icon icon-md" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round"><path stroke="none" d="M0 0h24v24H0z"/><path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2 -2v-2" /><polyline points="7 9 12 4 17 9" /><line x1="12" y1="4" x2="12" y2="16" /></svg>
                                </button>
                            </label>
                        </div>
                    </div>
                </div>);
    } // end of render()
} // end of class SaleableItemPicture

