import React from 'react';

import {BaseUrl} from '../js/constants.js';
import {toggle_visual_elm_showup, _instant_search} from '../js/common/native.js';
import {CommonPresentableProductItemForm} from './CommonPresentableProductItemForm.js';
import {TagsAppliedToItem} from '../components/TagsAppliedToItem.js'
import {SaleableItemPicture} from '../components/SaleableItemPicture.js'


let api_base_url = {
    item_plural:   {host:BaseUrl.API_HOST , path:'/product/saleableitems'},
    item_singular: {host:BaseUrl.API_HOST , path:'/product/saleableitem/{0}'},
};

let refs = {form_items: React.createRef()};

function _new_empty_form(evt) {
    let form_ref = this.current;
    form_ref.new_item(undefined, true);
}

function _save_items(evt) {
    let form_ref = this.current;
    let kwargs = {urlpath:api_base_url.item_plural.path , urlhost:api_base_url.item_plural.host, 
        fetch_fields:['id', 'tags', 'attributes'], };
    form_ref.save(kwargs);
}

class SaleableItems extends CommonPresentableProductItemForm {
    constructor(props) {
        let _valid_fields_name = ['visible', 'price', 'tags', 'ingredients',
            'pictures', 'videos']; 
        super(props, _valid_fields_name);
    }

    componentDidMount() {
        let attributes = [
            {id:71,  type:10,  value:"goat",},
            {id:75,  type:11,  value:2.711,},
            {id:123, type:12,  value:-29,},
        ];
        let tags = [
            {id:61, name:"no-one"},
            {id:65, name:"skid row"},
        ];
        let pictures = [
            {src:"./32t43094/t934it3rjf.jpg"},
            {src:"./093ur20u/g2h43gi3q4.jpg"},
            {src:"./RI49g3gf/MQOR43t34t.jpg"},
            {src:"./94jgij3m/bb9jrmrqjl.jpg"},
        ];
        let val = {name:'pee meat ball',  id:51  , visible: true, price: 25.04,
            attributes:attributes, tags:tags, pictures:pictures};
        this.new_item(val, true);
    }

    _normalize_fn_price(value) {
        return parseFloat(value);
    }
    _normalize_fn_visible(value) {
        return (value === "true" || value === true);
    }

    new_item(val, update_state) {
        var item = CommonPresentableProductItemForm.prototype.new_item.call(
                this, val, update_state);
        if(!item.name) {
            item.name = "<new saleable item>";
        }
        return item;
    }
    
    _single_item_visible_field_render(val, idx) {
        return (
            <label className="form-check">
                <input type="checkbox" className="form-check-input" name="example-text-input"
                    ref={val.refs.visible} defaultChecked={val.visible} />
                <span className="form-check-label"> Visible in front store </span>
            </label>
        );
    }
    
    _single_item_price_field_render(val, idx) {
        return (
            <label className="form-label">
                Base Price (excluding extra fee for chosen attributes)
                <input type="text" className="form-control" name="example-text-input"
                    ref={val.refs.price} defaultValue={val.price} />
            </label>
        );
    }
    _single_item_tags_field_render(val, idx) {
        return <label className="form-label">
                Tags applied to this item
                <TagsAppliedToItem  defaultValue={val.tags} ref={val.refs.tags} />
            </label> ;
    }
    _single_item_pictures_field_render(val, idx) {
        return <div className="col-md-6 col-xl-12 ">
                <label className="form-label"> Pictures</label>
                <SaleableItemPicture defaultValue={val.pictures} ref={val.refs.pictures} />
            </div> ;
    }
    _single_item_videos_field_render(val, idx) {
        return <> </>;
    }
    _single_item_ingredients_field_render(val, idx) {
        return <> </>;
    }

    _single_item_menu_render(val, idx) {
        let name_field    = this._single_item_name_field_render(val, idx);
        let visible_field = this._single_item_visible_field_render(val, idx);
        let price_field   = this._single_item_price_field_render(val, idx);
        let tags_field     = this._single_item_tags_field_render(val, idx);
        let pictures_field = this._single_item_pictures_field_render(val, idx);
        let videos_field   = this._single_item_videos_field_render(val, idx);
        let attributes_field  = this._single_item_attributes_field_render(val, idx);
        let ingredients_field = this._single_item_ingredients_field_render(val, idx);
        return (<>
                {visible_field} {name_field} {price_field} {tags_field}
                {pictures_field} {videos_field} {attributes_field} {ingredients_field}
            </>);
    } // end of _single_item_menu_render()
} // end of class SaleableItems


const SaleableItemsLayout = (props) => {
    let _sale_items = <SaleableItems ref={refs.form_items} />
    //let search_bound_obj = {search_api_fn: _instant_search_call_api};
    return (
        <>
          <div className="content">
          <div className="container-xl">
              <div className="row">
              <div className="col-xl-12">
                  <p>
                    all saleable items, attributes applied to them,
                    the media files to describe them, price estimate,
                    and price history of each item ...
                  </p>
                  <div className="card">
                    <div className="card-header">
                      <div className="d-flex">
                          <button className="btn btn-primary btn-pill ml-auto" onClick={ _new_empty_form.bind(refs.form_items) } >
                              new empty form
                          </button>
                          <button className="btn btn-primary btn-pill ml-auto" onClick={ _save_items.bind(refs.form_items) } >
                              Save
                          </button>
                      </div>
                    </div>
                    <div className="list list-row col-xl-8">
                        {_sale_items}
                    </div>
                  </div>
              </div>
              </div>
          </div>
          </div>
        </>
    );
};

export default SaleableItemsLayout;

