#include "helper_functions.h"


/*
 * Helper functions for tree
 */

xmlNodePtr xmlNextSibling(const xmlNodePtr cur) {
    return cur->next;
}

xmlNodePtr xmlPrevSibling(const xmlNodePtr cur) {
    return cur->prev;
}

xmlNodePtr xmlGetFirstChild(const xmlNodePtr cur) {
    return cur->children;
}

int xmlGetNodeType(const xmlNodePtr cur) {
    return cur->type;
}

const char * xmlNodeGetName(const xmlNodePtr cur) {
    return (char *) cur->name;
}

const char * xmlNodeGetContentPointer(const xmlNodePtr cur) {
    return (char *) cur->content;
}



/*
 * helper functions for xpath
 */

int xmlXPathObjectNumberOfNodes(const xmlXPathObjectPtr val) {
    if (!val) return -1;
    if (!val->nodesetval) return -2;
    return val->nodesetval->nodeNr;
}

xmlNodePtr xmlXPathObjectGetNode(const xmlXPathObjectPtr val, size_t index) {
    return val->nodesetval->nodeTab[index];
}

void xmlFreeXPathObject(xmlXPathObjectPtr val) {
    if (val->nodesetval) xmlFree(val->nodesetval->nodeTab);
    xmlFree(val->nodesetval);
    xmlFree(val);
}

/*
 * helper functions for parser
 */
static int hacky_well_formed = 0;

int htmlWellFormed(const htmlParserCtxtPtr ctxt) {
  return (((ctxt != NULL) && ctxt->wellFormed) || (hacky_well_formed));
}

// dummy function: no debug output at all
void _ignoreInvalidTagsErrorFunc(void * userData, xmlErrorPtr error) {
  if (error->code == XML_HTML_UNKNOWN_TAG) { // do not record invalid, in fact (out of despair) claim we ARE well-formed, when a tag is invalid.
    hacky_well_formed = 1;
  }
  return;
}
void setWellFormednessHandler(const htmlParserCtxtPtr ctxt) {
  hacky_well_formed = 0;
  xmlSetStructuredErrorFunc(ctxt, _ignoreInvalidTagsErrorFunc);
}