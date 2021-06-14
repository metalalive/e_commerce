import React from 'react';
import {BaseUrl, DEFAULT_API_REQUEST_OPTIONS} from '../js/constants.js';
import {BaseExtensibleForm} from '../js/common/react.js';

let api_base_url = {
    // acquire new access token signed in authentication server
    // , the access token returned by the API can be used in specific
    // resource service (in this case, it can be used in file-upload
    // service)
    remote_auth : {host:BaseUrl.API_HOST, path: '/usermgt/remote_auth'},
    // common API endpoint for uploading all types of files ,
    upload_singular: {host:BaseUrl.FILEUPLOAD_HOST , path: '/file/{0}'},
    // path to uploaded file on file-uploading server. Note the file will be deleted
    // asynchronously if it has never been referenced by any other application.
    uploaded_image : {host:BaseUrl.FILEUPLOAD_HOST, path: '/file/{0}?thumbnail'},
    // edit access control (ACL) of the uploaded file
    file_acl : {host:BaseUrl.FILEUPLOAD_HOST, path: '/file/{0}/acl'},
    // edit service reference  of the uploaded file
    file_referred : {host:BaseUrl.FILEUPLOAD_HOST, path: '/file/{0}/referenced_by'},
    // image paths stored in product service
    referred_product_image : {host:BaseUrl.API_HOST, path: '/product/saleable_item/{0}/image/{1}/thumbnail'},
};

let _fileupload_access_token = null;


export class SaleableItemPicture extends BaseExtensibleForm {
    constructor(props) {
        let _valid_fields_name = ['src',];
        super(props, _valid_fields_name);
    }
            
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
        // _fileupload_access_token = data.access_token;
        this._upload(_props);
    }

    _upload(props) {
        //props.access_token;
        //props.chosen_files;
        throw "not implemented yet";
    }

    _evt_upload_btn_click(e) {
        let props = {chosen_files: e.target.files};
        let access_token = _fileupload_access_token;
        if(access_token) {
            props.access_token = access_token;
            this._upload(props);
        } else {
            this._get_remote_access_token(props)
        }
    }

    _prompt_file_dialog(evt){
        let file_elm = document.createElement('input');
        file_elm.type = "file";
        file_elm.onchange = this._evt_upload_btn_click.bind(this);
        file_elm.click();
    }

    _single_item_render(val, idx) {
        return (
            <div className="col-6 col-sm-4">
                <label className="form-imagecheck mb-2">
                    <input name="form-imagecheck" type="checkbox" value="1" className="form-imagecheck-input" />
                    <span className="form-imagecheck-figure">
                        <img src={val.src} ref={val.refs.src} alt="product image" className="form-imagecheck-image" />
                    </span>
                </label>
            </div>);
    } // end of _single_item_render()

    render() {
        let uploaded_images = this.state.saved.map(this._single_item_render_wrapper.bind(this));
        return (
                <div className="mb-3">
                    <button className="btn btn-primary btn-pill ml-auto">Discard</button>
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

