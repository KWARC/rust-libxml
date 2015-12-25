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
