const supertest = require('supertest');
const expect = require('chai').expect;
const mocha = require('mocha');
const tv4 = require('tv4');
const fs = require('fs');
const pm = require('../../pm')

const baseUrl = supertest(pm.environment.get('server'));

const apiEndPoint = '/events/{{external_event_id}}';


var response;
var responseBody;


const put = async function (request_body) {
    return baseUrl
        .put(pm.substitute(apiEndPoint))
        .set('Accept', 'application/json')
        .set('Content-Type', 'application/json')
        .set('Authorization', pm.substitute('Bearer {{org_boxoffice_token}}'))

        .send(pm.substitute(request_body));
};

const get = async function (request_body) {
    return baseUrl
        .get(pm.substitute(apiEndPoint))

        .set('Authorization', pm.substitute('Bearer {{org_boxoffice_token}}'))

        .set('Accept', 'application/json')
        .send();
};

let requestBody = `{
    "name": "It's my party",
    "event_start": "2059-11-13T12:00:00",
    "override_status": "SoldOut"
}`;


describe('BoxOffice - Update External Event - 401', function () {
    before(async function () {
        response = await put(requestBody);
        console.log(response.request.header);
        console.log(response.request.url);
        console.log(response.request._data);
        console.log(response.request.method);
        responseBody = JSON.stringify(response.body);
        //console.log(pm);
        console.log(response.status);
        console.log(responseBody);
    });

    after(async function () {
        // add after methods


    });

    it("should be 401", function () {
        expect(response.status).to.equal(401);
    })


});

            