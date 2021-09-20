import random
import copy


def rand_gen_request_body(template, customize_item_fn, data_gen):
    def rand_gen_single_req(data):
        single_req_item = copy.deepcopy(template)
        single_req_item.update(data)
        customize_item_fn(single_req_item)
        return single_req_item
    return map(rand_gen_single_req, data_gen)


def listitem_rand_assigner(list_, min_num_chosen:int=2, max_num_chosen:int=-1, distinct:bool=True):
    # utility for testing
    assert any(list_), 'input list should not be empty'
    assert min_num_chosen >= 0, 'min_num_chosen = %s' % min_num_chosen
    num_avail = len(list_)
    if max_num_chosen > 0:
        err_msg = 'max_num_chosen = %s, min_num_chosen = %s' % (max_num_chosen, min_num_chosen)
        assert max_num_chosen > min_num_chosen, err_msg
        if max_num_chosen > (num_avail + 1) and distinct is True:
            err_msg = 'num_avail = %s, max_num_chosen = %s, distinct = %s' \
                    % (num_avail, max_num_chosen, distinct)
            raise ValueError(err_msg)
    else:
        err_msg =  'num_avail = %s , min_num_chosen = %s' % (num_avail, min_num_chosen)
        assert num_avail >= min_num_chosen, err_msg
        max_num_chosen = num_avail + 1
    if distinct:
        list_ = list(list_)
    num_assigned = random.randrange(min_num_chosen, max_num_chosen)
    for _ in range(num_assigned):
        idx = random.randrange(num_avail)
        yield list_[idx]
        if distinct:
            num_avail -= 1
            del list_[idx]
## end of listitem_rand_assigner


class HttpRequestDataGen:
    rand_create = True

    def customize_req_data_item(self, item, **kwargs):
        raise NotImplementedError()

    def refresh_req_data(self, fixture_source, http_request_body_template, num_create=None):
        if self.rand_create:
            kwargs = {'list_': fixture_source}
            if num_create:
                kwargs.update({'min_num_chosen': num_create, 'max_num_chosen':(num_create + 1)})
            data_gen = listitem_rand_assigner(**kwargs)
        else:
            num_create = num_create or len(fixture_source)
            data_gen = iter(fixture_source[:num_create])
        out = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=data_gen,  template=http_request_body_template)
        return list(out)


